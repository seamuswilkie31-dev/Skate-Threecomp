//! Original bounded triangle queries for the small imported prototype sample.
use crate::{
    geometry::Sample,
    skating::{Vec3, ground_height},
};

pub trait Terrain {
    fn height(&self, x: f32, z: f32, ceiling: f32) -> f32;
    fn blocks_motion(&self, from: Vec3, to: Vec3) -> bool;
}
pub struct World<'a> {
    pub mesh: Option<&'a TriangleMesh>,
}
impl Terrain for World<'_> {
    fn height(&self, x: f32, z: f32, ceiling: f32) -> f32 {
        let base = ground_height(x, z);
        self.mesh
            .and_then(|mesh| mesh.height(x, z, ceiling))
            .map_or(base, |y| y.max(base))
    }
    fn blocks_motion(&self, from: Vec3, to: Vec3) -> bool {
        self.mesh.is_some_and(|mesh| mesh.blocks_motion(from, to))
    }
}

#[derive(Debug, Clone)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    normal: Vec3,
    min: Vec3,
    max: Vec3,
}
#[derive(Debug, Clone)]
pub struct TriangleMesh {
    triangles: Vec<Triangle>,
    pub rideable_count: usize,
}
impl TriangleMesh {
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
    pub fn from_sample(sample: &Sample) -> Result<Self, String> {
        Self::from_sample_at(sample, |p| sample.preview_position(p))
    }
    pub fn from_sample_at(
        sample: &Sample,
        transform: impl Fn(Vec3) -> Vec3,
    ) -> Result<Self, String> {
        let mut faces = Vec::new();
        for mesh in &sample.meshes {
            if mesh.indices.len() % 3 != 0 {
                return Err("Collision index count is not a triangle list.".into());
            }
            for indices in mesh.indices.chunks_exact(3) {
                if faces.len() >= 20_000 {
                    return Err("Sample collision exceeds the triangle budget.".into());
                }
                let mut points = [Vec3::default(); 3];
                for i in 0..3 {
                    let p = *mesh
                        .positions
                        .get(indices[i] as usize)
                        .ok_or("Collision index is out of range.")?;
                    points[i] = transform(p);
                }
                faces.push(points);
            }
        }
        Self::from_triangles(&faces)
    }
    pub fn from_triangles(faces: &[[Vec3; 3]]) -> Result<Self, String> {
        if faces.len() > 20_000 {
            return Err("Collision triangle budget exceeded.".into());
        }
        let mut triangles = Vec::new();
        let mut rideable_count = 0;
        for &[a, b, c] in faces {
            if ![a, b, c].iter().all(|p| {
                [p.x, p.y, p.z]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
            }) {
                return Err("Invalid collision coordinate.".into());
            }
            let cross = (b - a).cross(c - a);
            if cross.dot(cross) < 1e-12 {
                continue;
            }
            let normal = cross.normalized();
            if normal.y.abs() >= 0.64 {
                rideable_count += 1;
            }
            let min = Vec3::new(
                a.x.min(b.x).min(c.x),
                a.y.min(b.y).min(c.y),
                a.z.min(b.z).min(c.z),
            );
            let max = Vec3::new(
                a.x.max(b.x).max(c.x),
                a.y.max(b.y).max(c.y),
                a.z.max(b.z).max(c.z),
            );
            triangles.push(Triangle {
                a,
                b,
                c,
                normal,
                min,
                max,
            });
        }
        if triangles.is_empty() {
            return Err("Collision sample contains no nondegenerate triangles.".into());
        }
        Ok(Self {
            triangles,
            rideable_count,
        })
    }
    /// Highest shallow triangle under the caller's vertical support ceiling.
    pub fn height(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
        if ![x, z, ceiling].iter().all(|v| v.is_finite()) {
            return None;
        }
        let mut best: Option<f32> = None;
        for t in &self.triangles {
            if t.normal.y.abs() < 0.64
                || x < t.min.x - 1e-5
                || x > t.max.x + 1e-5
                || z < t.min.z - 1e-5
                || z > t.max.z + 1e-5
            {
                continue;
            }
            let ab = t.b - t.a;
            let ac = t.c - t.a;
            let determinant = ab.x * ac.z - ab.z * ac.x;
            let dx = x - t.a.x;
            let dz = z - t.a.z;
            let u = (dx * ac.z - dz * ac.x) / determinant;
            let v = (ab.x * dz - ab.z * dx) / determinant;
            if u < -1e-5 || v < -1e-5 || u + v > 1.0 + 1e-5 {
                continue;
            }
            let y = t.a.y + u * ab.y + v * ac.y;
            if y <= ceiling + 1e-5 && best.is_none_or(|previous| y > previous) {
                best = Some(y);
            }
        }
        best
    }
    /// Three horizontal body-height probes, extended by a prototype radius.
    /// This is not a capsule solver or a full skateboard rigid-body collider.
    pub fn blocks_motion(&self, from: Vec3, to: Vec3) -> bool {
        let delta = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
        if delta.dot(delta) < 1e-10 {
            return false;
        }
        let extended = delta + delta.normalized() * 0.18;
        for t in &self.triangles {
            if t.normal.y.abs() >= 0.64
                || from.x.min(from.x + extended.x) > t.max.x
                || from.x.max(from.x + extended.x) < t.min.x
                || from.z.min(from.z + extended.z) > t.max.z
                || from.z.max(from.z + extended.z) < t.min.z
            {
                continue;
            }
            for height in [0.25, 0.9, 1.5] {
                let origin = from + Vec3::new(0.0, height - 0.12, 0.0);
                let edge1 = t.b - t.a;
                let edge2 = t.c - t.a;
                let h = extended.cross(edge2);
                let determinant = edge1.dot(h);
                if determinant.abs() < 1e-7 {
                    continue;
                }
                let s = origin - t.a;
                let u = s.dot(h) / determinant;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = s.cross(edge1);
                let v = extended.dot(q) / determinant;
                let fraction = edge2.dot(q) / determinant;
                if v >= 0.0 && u + v <= 1.0 && fraction > 1e-4 && fraction <= 1.0 {
                    return true;
                }
            }
        }
        false
    }
    /// A point above the highest substantial shallow triangle's centroid.
    pub fn placement(&self) -> Option<Vec3> {
        self.triangles
            .iter()
            .filter(|t| t.normal.y.abs() >= 0.64 && ((t.b - t.a).cross(t.c - t.a)).y.abs() > 0.08)
            .map(|t| (t.a + t.b + t.c) * (1.0 / 3.0))
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .map(|p| p + Vec3::new(0.0, 0.12, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skating::{FIXED_DT, Input, Skater};
    #[test]
    fn sloped_support_is_independent_of_triangle_winding() {
        let a = Vec3::new(0., 0., 0.);
        let b = Vec3::new(2., 1., 0.);
        let c = Vec3::new(0., 0., 2.);
        for triangle in [[a, b, c], [a, c, b]] {
            let mesh = TriangleMesh::from_triangles(&[triangle]).unwrap();
            assert!((mesh.height(0.5, 0.5, 2.).unwrap() - 0.25).abs() < 1e-6);
        }
    }
    #[test]
    #[ignore = "Requires THREECOMP_TEST_ISO pointing to the user's local retail disc image"]
    fn retail_sample_placement_supports_skater_and_ollie() {
        let path = std::env::var_os("THREECOMP_TEST_ISO").expect("Set THREECOMP_TEST_ISO");
        let sample = crate::geometry::load_retail_sample(std::path::Path::new(&path)).unwrap();
        let mesh = TriangleMesh::from_sample(&sample).unwrap();
        let placement = mesh.placement().expect("Sample has a placeable surface");
        let world = World { mesh: Some(&mesh) };
        let mut skater = Skater::default();
        skater.position = placement;
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && (skater.position.y - placement.y).abs() < 0.01);
        skater.step_on(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
            &world,
        );
        assert!(!skater.grounded);
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && (skater.position.y - placement.y).abs() < 0.01);
        assert_eq!(skater.landings, 1);
        eprintln!(
            "Retail collision: {} triangles / {} riding surfaces; placement {:?}",
            mesh.triangle_count(),
            mesh.rideable_count,
            placement
        );
    }
    fn roof() -> TriangleMesh {
        TriangleMesh::from_triangles(&[
            [
                Vec3::new(-2., 1., -2.),
                Vec3::new(2., 1., -2.),
                Vec3::new(-2., 1., 2.),
            ],
            [
                Vec3::new(2., 1., -2.),
                Vec3::new(2., 1., 2.),
                Vec3::new(-2., 1., 2.),
            ],
        ])
        .unwrap()
    }
    #[test]
    fn support_uses_triangle_interior_and_ceiling() {
        let mesh = roof();
        assert_eq!(mesh.height(0., 0., 2.), Some(1.));
        assert_eq!(mesh.height(0., 0., 0.5), None);
        assert_eq!(mesh.height(3., 0., 2.), None);
        assert_eq!(mesh.rideable_count, 2);
    }
    #[test]
    fn moving_body_sweeps_thin_walls_in_both_directions() {
        let mesh = TriangleMesh::from_triangles(&[[
            Vec3::new(0., 0., -2.),
            Vec3::new(0., 3., -2.),
            Vec3::new(0., 0., 2.),
        ]])
        .unwrap();
        assert!(mesh.blocks_motion(Vec3::new(-2., 0.12, 0.), Vec3::new(2., 0.12, 0.)));
        assert!(mesh.blocks_motion(Vec3::new(2., 0.12, 0.), Vec3::new(-2., 0.12, 0.)));
        assert!(!mesh.blocks_motion(Vec3::new(-0.2, 0.12, 0.), Vec3::new(-0.5, 0.12, 0.)));
        assert!(!mesh.blocks_motion(Vec3::new(-2., 4., 0.), Vec3::new(2., 4., 0.)));
    }
    #[test]
    fn falling_skater_lands_on_imported_surface_without_teleporting_up_from_below() {
        let mesh = roof();
        let world = World { mesh: Some(&mesh) };
        let mut skater = Skater::default();
        skater.position = Vec3::new(0., 3., 0.);
        skater.grounded = false;
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && (skater.position.y - 1.12).abs() < 0.001);
        skater.position.y = 0.12;
        skater.step_on(Input::default(), FIXED_DT, &world);
        assert!((skater.position.y - 0.12).abs() < 0.001);
    }
    #[test]
    fn invalid_and_degenerate_triangles_are_rejected() {
        assert!(TriangleMesh::from_triangles(&[[Vec3::default(); 3]]).is_err());
        assert!(TriangleMesh::from_triangles(&[[Vec3::new(f32::NAN, 0., 0.); 3]]).is_err());
    }
}
