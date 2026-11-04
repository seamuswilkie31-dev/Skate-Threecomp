# First verified material associations

The original bounded reader targets block 15 of the same presentation member as the [geometry sample](GEOMETRY.md). It reads one observed `0x00EB0005` parameter blob, its `0x00EB000B` reference table, and draw objects of type `0x00EB0023`. It does not execute shaders or reproduce the full game material system.

The parameter header specifies contiguous 12-byte group records followed by 32-byte parameter records and a string table. The reader validates counts, bounds, observed parameter types, profile fields, terminated UTF-8 names, and the asset-key parameters `Name`, `diffuse`, and `lightmap`. The initial reference rows have class `0x00EB0066` and must match the material groups in order and Name key. A draw's local material token is looked up in these rows rather than assumed to equal a group ordinal. Its mesh link is checked separately.

Two groups share a Name key but refer to different lightmaps. They remain distinct through their local tokens; deduplicating by Name would lose this information. The following direct observations are all zero-based object/group indexes:

| Draw object | Mesh object | Material group | Diffuse key | Loaded local texture block |
|---:|---:|---:|---|---:|
| 7 | 9 | 5 | `00008D0703E3870A` | 5 |
| 14 | 16 | 8 | `2C70170A000A000D` | Unresolved |
| 21 | 23 | 4 | `00008CFE03E3870A` | 4 |
| 28 | 30 | 7 | `00008D2F03E3870A` | Unresolved |
| 35 | 37 | 0 | `2C70170A00050227` | 7 |
| 42 | 44 | 3 | `2C70170A00050227` | 7 |
| 49 | 51 | 6 | `2C70170A00050227` | 7 |
| 56 | 58 | 2 | `0000804103E3870A` | Unresolved |
| 63 | 65 | 1 | `00008D1F03E3870A` | 6 |

Geometry's first UV set is a big-endian half-float pair at byte 12, declaration token `0x002C235F`. It supplies the diffuse coordinates. The native renderer normalizes the four decoded diffuse images to 64 × 64 texture-array layers using nearest duplication, then samples with repeating UVs and linear filtering. Layer zero is white for the original course and unresolved materials. The sample retains simple face shading and uses RGBA8Unorm; original lighting, gamma handling, normal/specular maps, mip filtering, alpha materials, and full shader behavior are not reconstructed. Declaration and numeric evidence support the UV format, but orientation, repeat/filter settings, and final appearance have not been compared with the original game. The projected CPU fallback remains plain.

Block 0's key `163899680E898770` appears as a **lightmap** reference. It is available in a separate pixel preview, never bound as a diffuse image. The second vertex attribute at byte 16 and lightmap-coordinate interpretation remain unverified, so the lightmap is not applied to meshes.

`--material-sample` reports all nine associations, four loaded textures, six textured meshes, and three plain meshes without exporting assets. Synthetic fixtures check malformed parameter strings and bounds, reference/group disagreements, nonordinal tokens, repeated Name keys, and invalid draw links. The native retail GPU check verifies that all six expected mesh bindings render without validation errors. Generalization to more members and resolving the three remaining diffuse keys are future work.
