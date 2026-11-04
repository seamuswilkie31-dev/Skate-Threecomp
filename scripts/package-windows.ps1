#requires -Version 7.0
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$PlanOnly,
    [string]$BinaryDirectory = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:OS -ne 'Windows_NT') { throw 'This package recipe requires Windows and PowerShell 7.' }
if ($BinaryDirectory -and -not $SkipBuild) { throw '-BinaryDirectory requires -SkipBuild.' }
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$targetTriple = 'x86_64-pc-windows-msvc'
$utf8 = [Text.UTF8Encoding]::new($false)

function Find-RustTool([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $candidate = Join-Path $env:USERPROFILE ".cargo\bin\$Name.exe"
    if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    throw "Required Rust tool not found: $Name. No tools are installed by this script."
}
function Invoke-Rust([string]$Tool, [string[]]$Arguments) {
    $result = & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw 'Rust command failed. See its diagnostic output.' }
    return $result
}
function Safe-Name([string]$Value) {
    if ($Value -notmatch '^[A-Za-z0-9][A-Za-z0-9+._-]*$') { throw 'An unsafe package or notice filename was encountered.' }
    return $Value
}
function Within([string]$Parent, [string]$Child) {
    $prefix = [IO.Path]::GetFullPath($Parent).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not [IO.Path]::GetFullPath($Child).StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'A package destination escaped its intended directory.'
    }
}
function Reject-Reparse([string]$Path) {
    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path -Force
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'Reparse points are not accepted in package inputs or output directories.'
        }
    }
}
function Check-Text([string]$Path) {
    # Notice prefixes are not file-type guarantees. Reject binary/image inputs
    # rather than letting replacement-character decoding disguise their bytes.
    if ([IO.Path]::GetExtension($Path) -in @('.iso', '.xex', '.p', '.exe', '.dll', '.pdb', '.png', '.jpg', '.jpeg', '.gif', '.webp', '.svg', '.ico', '.bmp', '.tif', '.tiff', '.tga', '.dds', '.ktx', '.ttf', '.otf', '.woff', '.woff2')) {
        throw 'An input with a binary, image, game-data, or font extension cannot be staged as text.'
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes -contains [byte]0) { throw 'A text input contains NUL bytes and cannot be staged as a notice.' }
    $content = [Text.UTF8Encoding]::new($false, $true).GetString($bytes)
    if ($content -match '(?i)([A-Z]:[\\/]Users[\\/]|[A-Z]:[\\/]Documents and Settings[\\/]|/Users/|/home/[^/\s]+/)') {
        throw 'A text input contains an absolute user-profile path; review it before packaging.'
    }
}
function Copy-SafeFile([string]$Source, [string]$Relative) {
    $destination = Join-Path $script:stageRoot $Relative
    Within $script:stageRoot $destination
    Reject-Reparse $Source
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination))
    [IO.File]::Copy($Source, $destination, $false)
}
function Write-NewText([string]$Path, [string]$Content) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try {
        $bytes = $script:utf8.GetBytes($Content.Replace("`r`n", "`n"))
        $stream.Write($bytes, 0, $bytes.Length)
    } finally { $stream.Dispose() }
}
function Check-X64Executable([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw 'Required release executable is missing. Build first or select the correct binary directory.' }
    Reject-Reparse $Path
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5A4D) { throw 'Release input is not a Windows executable.' }
        $stream.Position = 0x3C
        $offset = $reader.ReadUInt32()
        if ([uint64]$offset + 6 -gt [uint64]$stream.Length) { throw 'Invalid executable header bounds.' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x00004550 -or $reader.ReadUInt16() -ne 0x8664) { throw 'Release input is not an x64 PE executable.' }
    } finally { $reader.Dispose() }
}

$cargo = Find-RustTool 'cargo'
$rustc = Find-RustTool 'rustc'
$rustVersion = (Invoke-Rust -Tool $rustc -Arguments @('--version', '--verbose')) -join "`n"
if ($rustVersion -notmatch '(?m)^host: x86_64-pc-windows-msvc\s*$') { throw 'The active Rust host must be x86_64-pc-windows-msvc.' }
$cargoVersion = (Invoke-Rust -Tool $cargo -Arguments @('--version')) -join ' '
$metadataText = Invoke-Rust -Tool $cargo -Arguments @('metadata', '--offline', '--locked', '--format-version', '1', '--filter-platform', $targetTriple, '--manifest-path', (Join-Path $repositoryRoot 'Cargo.toml'))
$metadata = ($metadataText -join "`n") | ConvertFrom-Json
$packageMap = @{}
foreach ($package in $metadata.packages) { $packageMap[$package.id] = $package }
$launcher = @($metadata.packages | Where-Object { $_.name -eq 'skate-threecomp-launcher' })
if ($launcher.Count -ne 1) { throw 'Expected one launcher package in the locked workspace.' }
$nodeMap = @{}
foreach ($node in $metadata.resolve.nodes) { $nodeMap[$node.id] = $node }
$pending = [Collections.Generic.Stack[string]]::new()
$pending.Push($launcher[0].id)
$reachable = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
while ($pending.Count) {
    $id = $pending.Pop()
    if (-not $reachable.Add($id)) { continue }
    foreach ($dependency in $nodeMap[$id].deps) { $pending.Push($dependency.pkg) }
}
$packages = @($metadata.packages | Where-Object { $reachable.Contains($_.id) -and $_.id -notin $metadata.workspace_members } | Sort-Object name, version)
if ($packages.Count -gt 2000) { throw 'Dependency metadata exceeds the packaging budget.' }
$records = [Collections.Generic.List[object]]::new()
$noticeCopies = [Collections.Generic.List[object]]::new()
$folders = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$noticeBytes = 0L
foreach ($package in $packages) {
    $folder = Safe-Name "$($package.name)-$($package.version)"
    if (-not $folders.Add($folder)) { throw 'Different dependencies share a name/version; attribution folders need manual disambiguation.' }
    $root = [IO.Path]::GetDirectoryName($package.manifest_path)
    Reject-Reparse $root
    $issues = [Collections.Generic.List[string]]::new()
    $files = [Collections.Generic.List[object]]::new()
    foreach ($file in Get-ChildItem -LiteralPath $root -File -Force) {
        if ($file.Name -match '^(?i:LICENSE|LICENCE|COPYING|NOTICE)') { $files.Add($file) }
    }
    if ($package.license_file) {
        $declared = [IO.Path]::GetFullPath((Join-Path $root $package.license_file))
        if ([IO.Path]::GetDirectoryName($declared) -ne $root) {
            $issues.Add('Declared license_file is outside the package root; not copied by this recipe.')
        } elseif (-not (Test-Path -LiteralPath $declared -PathType Leaf)) {
            $issues.Add('Declared license_file is missing from the local cache.')
        } elseif ($declared -notin @($files | ForEach-Object FullName)) { $files.Add((Get-Item -LiteralPath $declared)) }
    }
    $included = [Collections.Generic.List[string]]::new()
    foreach ($file in @($files | Sort-Object Name -Unique)) {
        $name = Safe-Name $file.Name
        if ($file.Length -eq 0 -or $file.Length -gt 2MB) { $issues.Add("Notice file has an unsupported size: $name"); continue }
        Reject-Reparse $file.FullName
        Check-Text $file.FullName
        $noticeBytes += $file.Length
        if ($noticeBytes -gt 64MB) { throw 'Collected dependency notices exceed the packaging budget.' }
        $relative = "third-party/licenses/$folder/$name"
        $included.Add($relative)
        $noticeCopies.Add([pscustomobject]@{ Source = $file.FullName; Relative = $relative })
    }
    if (-not $included.Count) { $issues.Add('No root LICENSE/LICENCE/COPYING/NOTICE text was found in the local cached package.') }
    # Font bytes are embedded by this crate. Preserve their four separately
    # licensed notices via explicit paths, without copying font/image payloads.
    if ($package.name -eq 'epaint_default_fonts') {
        foreach ($fontNotice in @('fonts/OFL.txt', 'fonts/UFL.txt', 'fonts/Hack-Regular.txt', 'fonts/emoji-icon-font-mit-license.txt')) {
            $fontSource = Join-Path $root $fontNotice
            Within $root $fontSource
            if (-not (Test-Path -LiteralPath $fontSource -PathType Leaf)) { $issues.Add("Explicit embedded-font notice is missing: $fontNotice"); continue }
            Reject-Reparse (Join-Path $root 'fonts')
            Reject-Reparse $fontSource
            $fontFile = Get-Item -LiteralPath $fontSource
            if ($fontFile.Length -eq 0 -or $fontFile.Length -gt 2MB) { $issues.Add("Embedded-font notice size is unsupported: $fontNotice"); continue }
            Check-Text $fontSource
            $noticeBytes += $fontFile.Length
            if ($noticeBytes -gt 64MB) { throw 'Collected dependency notices exceed the packaging budget.' }
            $relative = "third-party/licenses/$folder/$fontNotice"
            $included.Add($relative)
            $noticeCopies.Add([pscustomobject]@{ Source = $fontSource; Relative = $relative })
        }
    }
    $expression = [string]$package.license
    if (-not $expression -or $expression -notmatch '^[A-Za-z0-9+(). _-]+$') {
        $expression = 'Unspecified; review included notice files'
        $issues.Add('License expression is missing or is outside the supported metadata format.')
    }
    $records.Add([ordered]@{ name = $package.name; version = $package.version; license_expression = $expression; notice_files = $included.ToArray(); review_notes = $issues.ToArray() })
}

$docsRoot = Join-Path $repositoryRoot 'docs'
$docCopies = [Collections.Generic.List[object]]::new()
$directories = [Collections.Generic.Stack[string]]::new()
$directories.Push($docsRoot)
while ($directories.Count) {
    $directory = $directories.Pop()
    Reject-Reparse $directory
    foreach ($item in Get-ChildItem -LiteralPath $directory -Force) {
        Reject-Reparse $item.FullName
        if ($item.PSIsContainer) { $directories.Push($item.FullName); continue }
        if ($item.Extension -notin @('.md', '.txt', '.rst')) { continue }
        if ($item.Length -gt 2MB -or $docCopies.Count -ge 1000) { throw 'Project documentation exceeds the packaging budget.' }
        Check-Text $item.FullName
        $relative = [IO.Path]::GetRelativePath($repositoryRoot, $item.FullName).Replace('\', '/')
        $docCopies.Add([pscustomobject]@{ Source = $item.FullName; Relative = $relative })
    }
}
foreach ($name in @('README.md', 'LICENSE')) {
    $source = Join-Path $repositoryRoot $name
    Check-Text $source
    $docCopies.Add([pscustomobject]@{ Source = $source; Relative = $name })
}
$missing = @($records | Where-Object { $_.review_notes.Count -gt 0 })
if ($PlanOnly) {
    [ordered]@{ target = $targetTriple; version = $launcher[0].version; third_party_packages = $records.Count; notice_files = $noticeCopies.Count; packages_needing_review = $missing.Count; project_text_files = $docCopies.Count; release_build_run = $false; output_written = $false } | ConvertTo-Json
    return
}

if (-not $SkipBuild) {
    Invoke-Rust -Tool $cargo -Arguments @('build', '--release', '--locked', '--offline', '--target', $targetTriple, '--package', 'skate-threecomp-launcher', '--bins', '--manifest-path', (Join-Path $repositoryRoot 'Cargo.toml')) | Out-Host
    $binaryRoot = Join-Path $metadata.target_directory "$targetTriple/release"
} elseif ($BinaryDirectory) {
    $binaryRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot $BinaryDirectory))
} else { $binaryRoot = Join-Path $metadata.target_directory 'release' }
$binaryNames = @('skate-threecomp.exe', 'skate-threecomp-inspect.exe')
foreach ($name in $binaryNames) { Check-X64Executable (Join-Path $binaryRoot $name) }

$commit = 'unknown'
$epoch = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
$dirty = $null
if (Get-Command git -ErrorAction SilentlyContinue) {
    $head = & git -C $repositoryRoot rev-parse HEAD 2>$null
    if ($LASTEXITCODE -eq 0 -and $head -match '^[0-9a-fA-F]{40,64}$') {
        $commit = [string]$head
        $seconds = & git -C $repositoryRoot show -s --format=%ct HEAD
        if ($LASTEXITCODE -ne 0) { throw 'Could not read the source commit time.' }
        $epoch = [DateTimeOffset]::FromUnixTimeSeconds([long]$seconds)
        $dirty = [bool](& git -C $repositoryRoot status --porcelain --untracked-files=normal)
    }
}
if ($epoch.Year -lt 1980 -or $epoch.Year -gt 2107) { throw 'Source timestamp is outside the ZIP timestamp range.' }
$label = Safe-Name ("Skate-Threecomp-v$($launcher[0].version)-$($commit.Substring(0, [Math]::Min(12, $commit.Length)))-windows-x64")
$dist = Join-Path $repositoryRoot 'dist'
Reject-Reparse $dist
$script:stageRoot = Join-Path $dist $label
Within $dist $script:stageRoot
$zipPath = Join-Path $dist "$label.zip"
$hashPath = "$zipPath.sha256"
foreach ($path in @($script:stageRoot, $zipPath, $hashPath)) {
    if (Test-Path -LiteralPath $path) { throw 'Package output already exists. Preserve or manually remove it before rerunning; this script never deletes output.' }
}
[void][IO.Directory]::CreateDirectory($script:stageRoot)
foreach ($name in $binaryNames) { Copy-SafeFile (Join-Path $binaryRoot $name) $name }
foreach ($copy in $docCopies.ToArray() + $noticeCopies.ToArray()) { Copy-SafeFile $copy.Source $copy.Relative }
[void][IO.Directory]::CreateDirectory((Join-Path $script:stageRoot 'third-party'))
$attribution = [ordered]@{
    schema_version = 1
    scope = 'Conservative Windows Cargo-metadata dependency closure, including build/dev entries returned for this package. Some entries may not be linked into the binaries.'
    collection = 'Existing package-root LICENSE/LICENCE/COPYING/NOTICE files, package-root declared license_file, and four explicit epaint_default_fonts/fonts text notices; no downloads or completeness claim.'
    dependencies = $records.ToArray()
}
Write-NewText (Join-Path $script:stageRoot 'third-party/dependencies.json') (($attribution | ConvertTo-Json -Depth 8) + "`n")
$reviewLines = [Collections.Generic.List[string]]::new()
$reviewLines.Add('Dependency attribution review report')
$reviewLines.Add('This is a file-collection report, not a claim that every notice, redistribution condition, or source obligation is satisfied.')
$reviewLines.Add('No missing notice text is downloaded or invented.')
$reviewLines.Add('')
if (-not $missing.Count) { $reviewLines.Add('Every collected dependency has at least one root notice file and a supported license metadata expression. Completeness still requires review.') }
foreach ($record in $missing) {
    $reviewLines.Add("$($record.name) $($record.version):")
    foreach ($note in $record.review_notes) { $reviewLines.Add("  - $note") }
}
Write-NewText (Join-Path $script:stageRoot 'third-party/REVIEW.txt') (($reviewLines -join "`n") + "`n")
$build = [ordered]@{ schema_version = 1; name = 'Skate Threecomp'; version = $launcher[0].version; source_commit = $commit; source_dirty = $dirty; target = $targetTriple; timestamp_utc = $epoch.UtcDateTime.ToString('yyyy-MM-ddTHH:mm:ssZ'); build_skipped = [bool]$SkipBuild; rustc = $rustVersion; cargo = $cargoVersion }
Write-NewText (Join-Path $script:stageRoot 'BUILD.json') (($build | ConvertTo-Json -Depth 4) + "`n")
Write-NewText (Join-Path $script:stageRoot 'QUICK_START.txt') @'
SKATE THREECOMP - PORTABLE WINDOWS PROTOTYPE

1. Unzip this entire folder, then run skate-threecomp.exe.
2. Click Open skating prototype to ride the original test course.
3. Rail practice: hold G (keyboard) or LT (controller), then Space / X to ollie.
   Use A/D or the left stick to balance on the rail; Space / X pops off.
4. W/S push/brake, A/D steer, Space ollie, F flip, Q/E air spin, R reset.
   Controller: A push, B brake, X ollie, Y flip, LB/RB spin, Start reset.

Your own ISO is needed for Load game mesh sample, Load neighborhood preview,
and Load texture sample. The neighborhood combines selected static batches;
Place on mesh starts on an imported riding surface.
Select it in the launcher before loading those samples. No game data is bundled.
The original game's full Play action remains unavailable. This is a native Rust
prototype and asset-research build, not a complete playable Skate 3 port.

skate-threecomp-inspect.exe is a command-line inspection tool.
See README.md and docs/GAMEPLAY.md for controls, scope, and current limitations.
'@

# Only files we explicitly staged can enter the archive. Stable ordering and
# commit-derived timestamps avoid cache timestamps and wall-clock variation.
$allFiles = @(Get-ChildItem -LiteralPath $script:stageRoot -File -Recurse)
$entryNames = [string[]]@($allFiles | ForEach-Object { [IO.Path]::GetRelativePath($script:stageRoot, $_.FullName).Replace('\', '/') })
[Array]::Sort($entryNames, [StringComparer]::Ordinal)
Add-Type -AssemblyName System.IO.Compression
$zipStream = [IO.File]::Open($zipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
$zip = [IO.Compression.ZipArchive]::new($zipStream, [IO.Compression.ZipArchiveMode]::Create, $false)
try {
    foreach ($relative in $entryNames) {
        $source = Join-Path $script:stageRoot $relative
        Within $script:stageRoot $source
        $entry = $zip.CreateEntry($relative, [IO.Compression.CompressionLevel]::Optimal)
        $entry.LastWriteTime = $epoch
        $input = [IO.File]::OpenRead($source)
        $output = $entry.Open()
        try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() }
    }
} finally { $zip.Dispose(); $zipStream.Dispose() }
$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
Write-NewText $hashPath "$hash  $label.zip`n"
Write-Host "Created dist/$label.zip"
Write-Host "SHA256: $hash"
Write-Host "Dependency records requiring review: $($missing.Count). Inspect third-party/REVIEW.txt before distribution."
