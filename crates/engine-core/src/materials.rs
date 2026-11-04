//! Original bounded reader for material associations in the observed retail
//! presentation arena. Asset keys are references, not resolved texture pixels.

use std::collections::HashMap;

use crate::rw4::Arena;

const MAX_MATERIALS: usize = 4096;
const MAX_PARAMETERS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialBinding {
    pub draw_index: usize,
    pub mesh_index: usize,
    /// Ordinal in the observed material group table.
    pub material_group: usize,
    /// Local reference token from the draw record and external-reference table.
    pub material_token: u32,
    pub material_key: u64,
    pub diffuse_key: Option<u64>,
    pub lightmap_key: Option<u64>,
}

#[derive(Debug)]
struct Material {
    key: u64,
    diffuse: Option<u64>,
    lightmap: Option<u64>,
}

/// Report associations only for the validated compact presentation profile.
/// It contains one EB0005 material blob and one EB000B reference table. The
/// initial material reference rows and material groups must agree in order and
/// key. This preserves distinct groups even when their Name keys are repeated.
pub fn inspect_materials(bytes: &[u8]) -> Result<Vec<MaterialBinding>, String> {
    let arena = Arena::read(bytes)?;
    let material_objects: Vec<_> = arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.type_id == 0xeb0005)
        .map(|(index, _)| index)
        .collect();
    let has_draws = arena
        .objects
        .iter()
        .any(|object| object.type_id == 0xeb0023);
    if material_objects.is_empty() && !has_draws {
        return Ok(Vec::new());
    }
    if material_objects.len() != 1 {
        return Err("Expected one observed material parameter object.".into());
    }
    let material_blob = arena.object_data(bytes, material_objects[0])?;
    let materials = read_parameter_groups(material_blob)?;
    let references: Vec<_> = arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.type_id == 0xeb000b)
        .map(|(index, _)| index)
        .collect();
    if references.len() != 1 {
        return Err("Expected one observed material reference table.".into());
    }
    let reference_blob = arena.object_data(bytes, references[0])?;
    let tokens = read_reference_tokens(reference_blob, &materials)?;
    let mut bindings = Vec::new();
    for (draw_index, _) in arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.type_id == 0xeb0023)
    {
        let draw = arena.object_data(bytes, draw_index)?;
        if draw.len() < 48 {
            return Err("Truncated material draw record.".into());
        }
        let token = word(draw, 36);
        let group = *tokens
            .get(&token)
            .ok_or("Draw material token is not in the validated reference table.")?;
        let mesh_index = word(draw, 44) as usize;
        let mesh_object = arena
            .objects
            .get(mesh_index)
            .ok_or("Material draw mesh index is out of range.")?;
        if mesh_object.type_id != 0x20081 || mesh_object.size != 16 {
            return Err("Material draw points to an unsupported mesh object.".into());
        }
        let material = &materials[group];
        bindings.push(MaterialBinding {
            draw_index,
            mesh_index,
            material_group: group,
            material_token: token,
            material_key: material.key,
            diffuse_key: material.diffuse,
            lightmap_key: material.lightmap,
        });
    }
    Ok(bindings)
}

fn read_parameter_groups(blob: &[u8]) -> Result<Vec<Material>, String> {
    if blob.len() < 20 {
        return Err("Material parameter header is truncated.".into());
    }
    let count = word(blob, 0) as usize;
    let parameter_count = word(blob, 4) as usize;
    let table = word(blob, 8) as usize;
    let parameter_base = word(blob, 12) as usize;
    let string_base = word(blob, 16) as usize;
    if count == 0 || count > MAX_MATERIALS || parameter_count > MAX_PARAMETERS {
        return Err("Material or parameter count exceeds the observed profile budget.".into());
    }
    let group_end = table
        .checked_add(count * 12)
        .ok_or("Material group table size overflow.")?;
    let parameter_end = parameter_base
        .checked_add(parameter_count * 32)
        .ok_or("Material parameter table size overflow.")?;
    if table != 20
        || group_end != parameter_base
        || parameter_end != string_base
        || string_base > blob.len()
    {
        return Err("Material parameter tables overlap or exceed their object.".into());
    }
    let mut expected_start = parameter_base;
    let mut materials = Vec::with_capacity(count);
    for index in 0..count {
        let group = table + index * 12;
        let group_count = word(blob, group) as usize;
        let start = word(blob, group + 8) as usize;
        let group_bytes = group_count
            .checked_mul(32)
            .ok_or("Material parameter group size overflow.")?;
        let end = start
            .checked_add(group_bytes)
            .ok_or("Material parameter group size overflow.")?;
        if group_count == 0
            || group_count > MAX_PARAMETERS
            || word(blob, group + 4) != 0
            || start != expected_start
            || end > string_base
        {
            return Err("Material group parameters are noncontiguous or out of bounds.".into());
        }
        expected_start = end;
        let mut key = None;
        let mut diffuse = None;
        let mut lightmap = None;
        for parameter in 0..group_count {
            let record = start + parameter * 32;
            let name = string(blob, word(blob, record) as usize, string_base, 256)?;
            let kind = word(blob, record + 4);
            if !matches!(kind, 0xffff | 0x20000)
                || word(blob, record + 8) != 0xdeadbeef
                || word(blob, record + 12) != 0xdeadc0de
                || word(blob, record + 28) != 0
            {
                return Err("Material parameter has unsupported type or profile fields.".into());
            }
            let display_name = word(blob, record + 24) as usize;
            if display_name != 0 {
                string(blob, display_name, string_base, 4096)?;
            }
            let value = key64(blob, record + 16);
            let destination = match name {
                "Name" => Some(&mut key),
                "diffuse" => Some(&mut diffuse),
                "lightmap" => Some(&mut lightmap),
                _ => None,
            };
            if let Some(destination) = destination
                && (kind != 0xffff || destination.replace(value).is_some())
            {
                return Err(
                    "Material asset-key parameter is duplicated or has the wrong type.".into(),
                );
            }
        }
        materials.push(Material {
            key: key.ok_or("Material group has no Name asset key.")?,
            diffuse,
            lightmap,
        });
    }
    if expected_start != string_base {
        return Err("Material parameter count does not match the group tables.".into());
    }
    Ok(materials)
}

fn read_reference_tokens(
    blob: &[u8],
    materials: &[Material],
) -> Result<HashMap<u32, usize>, String> {
    if blob.len() < 20 {
        return Err("Material reference header is truncated.".into());
    }
    let count = word(blob, 0) as usize;
    let table = word(blob, 4) as usize;
    if count < materials.len() || count > MAX_PARAMETERS || table != 20 {
        return Err("Material reference count or table offset is unsupported.".into());
    }
    let end = table
        .checked_add(count * 24)
        .ok_or("Material reference table size overflow.")?;
    if end > blob.len() || word(blob, 8) as usize != end {
        return Err("Material reference records exceed their declared table.".into());
    }
    let mut tokens = HashMap::with_capacity(materials.len());
    for (index, material) in materials.iter().enumerate() {
        let record = table + index * 24;
        let token = word(blob, record + 20);
        if word(blob, record) != 0
            || word(blob, record + 4) != 0xfeffffff
            || word(blob, record + 16) != 0xeb0066
            || key64(blob, record + 8) != material.key
            || token & 0xff800000 != 0x00800000
            || tokens.insert(token, index).is_some()
        {
            return Err(
                "Material reference/group order, keys, or local tokens do not match.".into(),
            );
        }
    }
    // Later references may describe other classes, but not extra material groups.
    for index in materials.len()..count {
        if word(blob, table + index * 24 + 16) == 0xeb0066 {
            return Err("Uncatalogued material reference follows the validated groups.".into());
        }
    }
    Ok(tokens)
}

fn string(blob: &[u8], offset: usize, base: usize, limit: usize) -> Result<&str, String> {
    if offset < base || offset >= blob.len() {
        return Err("Material string points outside its string table.".into());
    }
    let remaining = &blob[offset..blob.len().min(offset.saturating_add(limit + 1))];
    let end = remaining
        .iter()
        .position(|&byte| byte == 0)
        .ok_or("Material string is too long or not terminated.")?;
    let value =
        std::str::from_utf8(&remaining[..end]).map_err(|_| "Material string is not UTF-8.")?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("Material string is empty or contains control characters.".into());
    }
    Ok(value)
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn key64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    fn parameters() -> Vec<u8> {
        let mut blob = vec![0; 164];
        // Header + one group + three parameters occupy bytes 0..128.
        for (offset, value) in [
            (0, 1),
            (4, 3),
            (8, 20),
            (12, 32),
            (16, 128),
            (20, 3),
            (28, 32),
        ] {
            put(&mut blob, offset, value);
        }
        blob[128..150].copy_from_slice(b"Name\0diffuse\0lightmap\0");
        for (index, name) in [128, 133, 141].into_iter().enumerate() {
            let record = 32 + index * 32;
            for (offset, value) in [
                (0, name),
                (4, 0xffff),
                (8, 0xdeadbeef),
                (12, 0xdeadc0de),
                (20, index as u32 + 1),
                (24, 150),
            ] {
                put(&mut blob, record + offset, value);
            }
        }
        blob[150..154].copy_from_slice(b"key\0");
        blob
    }
    fn references() -> Vec<u8> {
        let mut blob = vec![0; 44];
        for (offset, value) in [
            (0, 1),
            (4, 20),
            (8, 44),
            (24, 0xfeffffff),
            (32, 1),
            (36, 0xeb0066),
            (40, 0x800009),
        ] {
            put(&mut blob, offset, value);
        }
        blob
    }
    fn arena_fixture() -> Vec<u8> {
        let mut bytes = vec![0; 544];
        bytes[..24].copy_from_slice(b"\x89RW4xb2\0\r\n\x1a\n\x01\x20\x04\x00454\x00000\x00");
        for (offset, value) in [
            (0x20, 4),
            (0x24, 4),
            (0x30, 448),
            (0x44, 544),
            (0x48, 4),
            (0x50, 1),
            (0x58, 1),
            (0x60, 1),
            (0x68, 1),
        ] {
            put(&mut bytes, offset, value);
        }
        bytes[172..336].copy_from_slice(&parameters());
        bytes[336..380].copy_from_slice(&references());
        put(&mut bytes, 380 + 36, 0x800009);
        put(&mut bytes, 380 + 44, 3);
        for (index, (offset, size, kind)) in [
            (172, 164, 0xeb0005),
            (336, 44, 0xeb000b),
            (380, 48, 0xeb0023),
            (428, 16, 0x20081),
        ]
        .into_iter()
        .enumerate()
        {
            let record = 448 + index * 24;
            for (field, value) in [(0, offset), (8, size), (12, 4), (20, kind)] {
                put(&mut bytes, record + field, value);
            }
        }
        bytes
    }
    #[test]
    fn bounded_arena_reader_resolves_draw_and_rejects_bad_draw_links() {
        let bindings = inspect_materials(&arena_fixture()).unwrap();
        assert_eq!(
            bindings,
            [MaterialBinding {
                draw_index: 2,
                mesh_index: 3,
                material_group: 0,
                material_token: 0x800009,
                material_key: 1,
                diffuse_key: Some(2),
                lightmap_key: Some(3)
            }]
        );
        for (offset, value) in [(416, 0x800000), (424, 99), (540, 0x200e8), (172 + 32, 20)] {
            let mut bytes = arena_fixture();
            put(&mut bytes, offset, value);
            assert!(inspect_materials(&bytes).is_err());
        }
    }
    #[test]
    fn reads_named_keys_and_resolves_nonordinal_local_token() {
        let groups = read_parameter_groups(&parameters()).unwrap();
        assert_eq!(groups[0].key, 1);
        assert_eq!(groups[0].diffuse, Some(2));
        assert_eq!(groups[0].lightmap, Some(3));
        let tokens = read_reference_tokens(&references(), &groups).unwrap();
        assert_eq!(tokens.get(&0x800009), Some(&0));
        assert!(!tokens.contains_key(&0x800000));
    }
    #[test]
    fn rejects_offsets_counts_unterminated_names_and_wrong_parameter_types() {
        for (offset, value) in [
            (0, u32::MAX),
            (16, u32::MAX),
            (20, u32::MAX),
            (28, 36),
            (32, 20),
            (36, 0x20000),
            (40, 0),
            (64, 128),
        ] {
            let mut blob = parameters();
            put(&mut blob, offset, value);
            assert!(read_parameter_groups(&blob).is_err());
        }
        let mut blob = parameters();
        blob[128..].fill(b'x');
        assert!(read_parameter_groups(&blob).is_err());
    }
    #[test]
    fn rejects_reference_key_order_type_and_token_mismatches() {
        let groups = read_parameter_groups(&parameters()).unwrap();
        for (offset, value) in [(0, 0), (4, 24), (8, 40), (32, 9), (36, 0xeb0005), (40, 0)] {
            let mut blob = references();
            put(&mut blob, offset, value);
            assert!(read_reference_tokens(&blob, &groups).is_err());
        }
    }
    #[test]
    fn repeated_name_keys_keep_distinct_groups_via_the_reference_tokens() {
        let groups = vec![
            Material {
                key: 5,
                diffuse: Some(10),
                lightmap: Some(20),
            },
            Material {
                key: 5,
                diffuse: Some(10),
                lightmap: Some(30),
            },
        ];
        let mut refs = vec![0; 68];
        for (offset, value) in [(0, 2), (4, 20), (8, 68)] {
            put(&mut refs, offset, value);
        }
        for (index, token) in [0x800003, 0x800000].into_iter().enumerate() {
            let record = 20 + index * 24;
            for (offset, value) in [(4, 0xfeffffff), (12, 5), (16, 0xeb0066), (20, token)] {
                put(&mut refs, record + offset, value);
            }
        }
        let tokens = read_reference_tokens(&refs, &groups).unwrap();
        assert_eq!(groups[tokens[&0x800000]].lightmap, Some(30));
        assert_eq!(groups[tokens[&0x800003]].lightmap, Some(20));
    }
}
