# Retail metadata profile

## Skate 3 Xbox 360 retail disc (USA/Europe metadata)

The read-only source inspector recognizes this XEX execution metadata combination:

| Field | Value |
|---|---:|
| Title ID | `0x454108E6` |
| Media ID | `0x5C087C2C` |
| Version field | `0x00000003` |
| Base version field | `0x00000003` |

The title and media identifiers are cross-referenced against the [DBox Skate 3 title listing](https://dbox.tools/titles/xbox360/454108E6/). The USA/Europe media identifier is also noted in the [Xenia Canary Skate 3 game profile](https://github.com/xenia-canary/game-patches/blob/main/patches/454108E6%20-%20Skate%203.patch.toml). These are external metadata references; no source code from Skate 3 Recompiled or another game implementation is used by this project.

This match identifies a metadata profile only. The inspector currently does not compare a cryptographic hash of `default.xex`, validate every file in the image, or run the game. A profile match must not be read as confirmation that the image is supported or playable. Other regions, title updates, and modified images may report different metadata and are not yet catalogued.

The inspected user image currently reports these same four fields. Inspection is read-only and does not extract or modify game files.
