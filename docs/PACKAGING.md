# Portable Windows packaging

`scripts/package-windows.ps1` provides a bounded packaging recipe for the x64 MSVC Windows build. It requires PowerShell 7, the existing Rust toolchain, and the dependencies already available in the local Cargo cache. It does not install tools, download notices, change settings, publish releases, or delete existing output.

From the repository root:

```powershell
pwsh -NoProfile -File scripts/package-windows.ps1 -PlanOnly
pwsh -NoProfile -File scripts/package-windows.ps1
```

The default invocation builds the launcher package's binaries with `--release --locked --offline --target x86_64-pc-windows-msvc`. It stages the portable files and creates a ZIP plus a SHA256 file under `dist/`. The active Rust host must be `x86_64-pc-windows-msvc`; both executables are checked for x64 PE headers before staging.

`-PlanOnly` runs the offline locked metadata/cache and input checks, prints counts, and returns without building or writing package artifacts. If dependencies are not cached, the recipe stops with Cargo's diagnostic rather than fetching them.

## Reusing an existing build

If the caller has already built and verified fresh host release binaries:

```powershell
cargo build --release --locked --offline --workspace
pwsh -NoProfile -File scripts/package-windows.ps1 -SkipBuild -BinaryDirectory target/release
```

The default `-SkipBuild` directory is the Cargo target directory's `release` folder. For the explicit target layout produced by the default package command, supply `-BinaryDirectory target/x86_64-pc-windows-msvc/release`. The caller is responsible for binary freshness when skipping the build; PE validation establishes architecture, not correspondence with the current source commit.

## Included files

The archive contains only these staged inputs and generated reports:

- `skate-threecomp.exe` and `skate-threecomp-inspect.exe`.
- The project's `README.md`, `LICENSE`, and `.md`/`.txt`/`.rst` documentation.
- `BUILD.json` with package version, source commit/dirty state, target, tool versions, commit-derived timestamp, and whether the build was skipped.
- `QUICK_START.txt` with portable launch, prototype controls, rail practice, and ISO/full-game scope notes.
- `third-party/dependencies.json`, listing dependency crate names, versions, license metadata expressions, copied notice filenames, and review notes.
- Existing dependency-package root files whose names begin with `LICENSE`, `LICENCE`, `COPYING`, or `NOTICE`; declared `license_file` entries are included only when they also reside in the package root.
- The four explicit `epaint_default_fonts/fonts/` text notices (`OFL.txt`, `UFL.txt`, `Hack-Regular.txt`, and `emoji-icon-font-mit-license.txt`) for embedded font assets. Font binaries are not separately staged.
- `third-party/REVIEW.txt`, flagging missing notice files, absent/unsupported license expressions, or excluded notice cases.

Raw Cargo metadata, registry paths, user source paths, game images/executables, saves, decoded pixels, screenshots, PDB files, and installers are not staged. Project documentation is restricted to text extensions; dependency inputs are restricted to root notice files and the four explicit font notices. Text must be valid UTF-8 without NUL bytes; binary, game-data, image, and font extensions are rejected. Absolute user-profile paths in text inputs cause packaging to stop for review rather than copying them into an archive.

## Attribution and reproducibility limits

The dependency list conservatively follows the Windows-filtered Cargo metadata closure for the launcher, including build/dev edges returned by Cargo. Some listed crates may not be linked into the binaries. Notice collection includes an explicit bounded exception for the embedded default-font notices, but does not establish that every notice or redistribution/source requirement has been met; the review report explicitly makes no completeness claim. Missing root notices are still flagged even when a font notice is included. Missing text is flagged, never fabricated or automatically downloaded. Review the report and included license texts before distributing a package.

ZIP entries are sorted and use the source commit timestamp, reducing variation from filesystem cache times and the packaging wall clock. This is a repeatable recipe, not a guarantee of byte-identical binaries across compiler/linker or compression runtime versions. Dirty source state is recorded; source commit identity alone does not identify uncommitted changes. If Git metadata is unavailable, the report records an unknown commit and uses the ZIP epoch.

The staged directory is retained next to the ZIP for inspection. If a destination already exists, the script stops; it does not recursively remove, move, or overwrite it. Preserve or manually clean old output before rerunning, and keep `dist/` out of source commits. No GitHub release or other publication is performed.
