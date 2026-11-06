//! Original bounded triangle queries for the small imported prototype sample.
use crate::{
    geometry::Sample,
    rails::{BOARD_HEIGHT, COURSE_RAILS, Rail},
    skating::{COURSE_X, COURSE_Z, Vec3, ground_height},
};

pub const BODY_HEAD_HEIGHT: f32 = 1.65;
const BODY_RADIUS: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub fraction: f32,
    pub point: Vec3,
    /// Unit normal facing against the query direction, independent of winding.
    pub normal: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub position: Vec3,
    pub blocked: bool,
}

fn valid_point(point: Vec3) -> bool {
    [point.x, point.y, point.z]
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
}

pub trait Terrain {
    fn height(&self, x: f32, z: f32, ceiling: f32) -> f32;
    fn support(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
        Some(self.height(x, z, ceiling))
    }
    fn contains_motion(&self, _from: Vec3, to: Vec3) -> bool {
        to.x.is_finite()
            && to.z.is_finite()
            && to.x.abs() <= COURSE_X - 0.5
            && to.z.abs() <= COURSE_Z - 0.5
    }
    fn rails(&self) -> &[Rail] {
        &[]
    }
    fn raycast(&self, _from: Vec3, _to: Vec3) -> Option<RayHit> {
        None
    }
    fn ceiling_hit(&self, _from_board: Vec3, _to_board: Vec3) -> Option<RayHit> {
        None
    }
    fn resolve_motion(&self, from: Vec3, to: Vec3) -> Motion {
        if !valid_point(from)
            || !valid_point(to)
            || !self.contains_motion(from, to)
            || self.blocks_motion(from, to)
        {
            Motion {
                position: from,
                blocked: true,
            }
        } else {
            Motion {
                position: to,
                blocked: false,
            }
        }
    }
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
    fn rails(&self) -> &[Rail] {
        &COURSE_RAILS
    }
    fn raycast(&self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.mesh.and_then(|mesh| mesh.raycast(from, to))
    }
    fn ceiling_hit(&self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.mesh.and_then(|mesh| mesh.ceiling_hit(from, to))
    }
    fn resolve_motion(&self, from: Vec3, to: Vec3) -> Motion {
        if !valid_point(from) || !valid_point(to) || !self.contains_motion(from, to) {
            return Motion {
                position: from,
                blocked: true,
            };
        }
        let motion = self.mesh.map_or(
            Motion {
                position: to,
                blocked: false,
            },
            |mesh| mesh.resolve_motion(from, to),
        );
        if self.contains_motion(from, motion.position) {
            motion
        } else {
            Motion {
                position: from,
                blocked: true,
            }
        }
    }
}

/// Source geometry alone supplies support; gaps never acquire a synthetic floor.
/// Invalid bounds make support and movement unavailable rather than unbounded.
pub struct SourceWorld<'a> {
    pub mesh: &'a TriangleMesh,
    pub min: Vec3,
    pub max: Vec3,
}
impl SourceWorld<'_> {
    fn contains_xz(&self, x: f32, z: f32) -> bool {
        [self.min, self.max]
            .iter()
            .all(|p| [p.x, p.y, p.z].iter().all(|v| v.is_finite()))
            && self.min.x <= self.max.x
            && self.min.y <= self.max.y
            && self.min.z <= self.max.z
            && x.is_finite()
            && z.is_finite()
            && (self.min.x..=self.max.x).contains(&x)
            && (self.min.z..=self.max.z).contains(&z)
    }
}
impl Terrain for SourceWorld<'_> {
    fn height(&self, x: f32, z: f32, ceiling: f32) -> f32 {
        self.support(x, z, ceiling).unwrap_or(f32::NEG_INFINITY)
    }
    fn support(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
        self.contains_xz(x, z)
            .then(|| self.mesh.height(x, z, ceiling))
            .flatten()
    }
    fn contains_motion(&self, _from: Vec3, to: Vec3) -> bool {
        to.y.is_finite() && self.contains_xz(to.x, to.z)
    }
    fn blocks_motion(&self, from: Vec3, to: Vec3) -> bool {
        self.mesh.blocks_motion(from, to)
    }
    fn raycast(&self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.mesh.raycast(from, to)
    }
    fn ceiling_hit(&self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.mesh.ceiling_hit(from, to)
    }
    fn resolve_motion(&self, from: Vec3, to: Vec3) -> Motion {
        if !valid_point(from) || !valid_point(to) || !self.contains_motion(from, to) {
            return Motion {
                position: from,
                blocked: true,
            };
        }
        let motion = self.mesh.resolve_motion(from, to);
        if self.contains_motion(from, motion.position) {
            motion
        } else {
            Motion {
                position: from,
                blocked: true,
            }
        }
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
impl Triangle {
    fn unit_normal(&self) -> Vec3 {
        self.normal * (1.0 / self.normal.dot(self.normal).sqrt())
    }
    fn contains_point(&self, point: Vec3) -> bool {
        let ab = self.b - self.a;
        let ac = self.c - self.a;
        let area = ab.cross(ac).dot(self.normal);
        let relative = point - self.a;
        let u = relative.cross(ac).dot(self.normal) / area;
        let v = ab.cross(relative).dot(self.normal) / area;
        u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5
    }
    fn segment_hit(&self, from: Vec3, delta: Vec3) -> Option<RayHit> {
        let normal = self.unit_normal();
        let denominator = normal.dot(delta);
        if denominator.abs() < 1e-7 {
            return None;
        }
        let fraction = normal.dot(self.a - from) / denominator;
        if !(-1e-5..=1.0 + 1e-5).contains(&fraction) {
            return None;
        }
        let fraction = fraction.clamp(0.0, 1.0);
        let point = from + delta * fraction;
        self.contains_point(point).then_some(RayHit {
            fraction,
            point,
            normal: if denominator > 0.0 {
                normal * -1.0
            } else {
                normal
            },
        })
    }
    fn vertical_height(&self, x: f32, z: f32) -> Option<f32> {
        if x < self.min.x - 1e-5
            || x > self.max.x + 1e-5
            || z < self.min.z - 1e-5
            || z > self.max.z + 1e-5
        {
            return None;
        }
        let ab = self.b - self.a;
        let ac = self.c - self.a;
        let determinant = ab.x * ac.z - ab.z * ac.x;
        if determinant.abs() < 1e-8 {
            return None;
        }
        let dx = x - self.a.x;
        let dz = z - self.a.z;
        let u = (dx * ac.z - dz * ac.x) / determinant;
        let v = (ab.x * dz - ab.z * dx) / determinant;
        if u < -1e-5 || v < -1e-5 || u + v > 1.0 + 1e-5 {
            return None;
        }
        Some(self.a.y + u * ab.y + v * ac.y)
    }
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
            if t.normal.y.abs() < 0.64 {
                continue;
            }
            let Some(y) = t.vertical_height(x, z) else {
                continue;
            };
            if y <= ceiling + 1e-5 && best.is_none_or(|previous| y > previous) {
                best = Some(y);
            }
        }
        best
    }
    /// Nearest two-sided contact along a finite segment, including its endpoints.
    /// Zero-length, non-finite, or excessive coordinates return no contact.
    pub fn raycast(&self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.raycast_filtered(from, to, |_| true)
    }
    fn raycast_filtered(
        &self,
        from: Vec3,
        to: Vec3,
        accept: impl Fn(&RayHit) -> bool,
    ) -> Option<RayHit> {
        if !valid_point(from) || !valid_point(to) {
            return None;
        }
        let delta = to - from;
        if delta.dot(delta) < 1e-12 {
            return None;
        }
        let mut nearest: Option<RayHit> = None;
        for triangle in &self.triangles {
            if from.x.min(to.x) > triangle.max.x + 1e-5
                || from.x.max(to.x) < triangle.min.x - 1e-5
                || from.y.min(to.y) > triangle.max.y + 1e-5
                || from.y.max(to.y) < triangle.min.y - 1e-5
                || from.z.min(to.z) > triangle.max.z + 1e-5
                || from.z.max(to.z) < triangle.min.z - 1e-5
            {
                continue;
            }
            if let Some(hit) = triangle.segment_hit(from, delta)
                && accept(&hit)
                && nearest.is_none_or(|previous| hit.fraction < previous.fraction)
            {
                nearest = Some(hit);
            }
        }
        nearest
    }
    /// Sweep the head from its pre-step position only while the board rises.
    /// The returned point is the head contact; no depenetration is attempted.
    pub fn ceiling_hit(&self, from_board: Vec3, to_board: Vec3) -> Option<RayHit> {
        if !valid_point(from_board) || !valid_point(to_board) || to_board.y <= from_board.y {
            return None;
        }
        self.raycast_filtered(
            from_board + Vec3::new(0.0, BODY_HEAD_HEIGHT, 0.0),
            to_board + Vec3::new(0.0, BODY_HEAD_HEIGHT, 0.0),
            |hit| hit.normal.y < -1e-5,
        )
    }
    /// Resolve horizontal body motion against steep faces with three probes and
    /// a 0.18-unit horizontal radius. At most two contacts preserve tangential
    /// displacement. Vertical movement remains the caller's separate operation.
    pub fn resolve_motion(&self, from: Vec3, to: Vec3) -> Motion {
        if !valid_point(from) || !valid_point(to) {
            return Motion {
                position: from,
                blocked: true,
            };
        }
        let mut position = from;
        let mut remaining = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
        let requested_length_squared = remaining.dot(remaining);
        let mut blocked = false;
        for _ in 0..2 {
            if remaining.dot(remaining) < 1e-12 {
                break;
            }
            let Some(hit) = self.body_hit(position, remaining) else {
                position = position + remaining;
                break;
            };
            blocked = true;
            position = position + remaining * hit.fraction;
            remaining = remaining * (1.0 - hit.fraction);
            let inward = remaining.dot(hit.normal).min(0.0);
            remaining = remaining - hit.normal * inward;
        }
        let actual = Vec3::new(position.x - from.x, 0.0, position.z - from.z);
        let actual_length_squared = actual.dot(actual);
        if actual_length_squared > requested_length_squared {
            position = from + actual * (requested_length_squared / actual_length_squared).sqrt();
        }
        position.y = to.y;
        Motion { position, blocked }
    }
    fn body_hit(&self, from: Vec3, delta: Vec3) -> Option<RayHit> {
        let to = from + delta;
        let mut nearest: Option<RayHit> = None;
        for triangle in &self.triangles {
            let normal = triangle.unit_normal();
            if normal.y.abs() >= 0.64
                || from.x.min(to.x) > triangle.max.x + BODY_RADIUS
                || from.x.max(to.x) < triangle.min.x - BODY_RADIUS
                || from.z.min(to.z) > triangle.max.z + BODY_RADIUS
                || from.z.max(to.z) < triangle.min.z - BODY_RADIUS
            {
                continue;
            }
            let horizontal = Vec3::new(normal.x, 0.0, normal.z);
            let horizontal_length = horizontal.dot(horizontal).sqrt();
            let horizontal = horizontal * (1.0 / horizontal_length);
            for height in [0.25, 0.9, 1.5] {
                let origin = from + Vec3::new(0.0, height - BOARD_HEIGHT, 0.0);
                let distance = normal.dot(origin - triangle.a) / horizontal_length;
                // A center exactly on the plane has no known safe side. Let it
                // escape rather than inventing a depenetration direction.
                if distance.abs() <= 1e-6 {
                    continue;
                }
                let outward = horizontal * distance.signum();
                let approach = delta.dot(outward);
                if approach >= -1e-7 {
                    continue;
                }
                let fraction = ((BODY_RADIUS - distance.abs()) / approach).max(0.0);
                if fraction > 1.0 || nearest.is_some_and(|hit| hit.fraction <= fraction) {
                    continue;
                }
                let center = origin + delta * fraction;
                let plane_distance = normal.dot(center - triangle.a) / horizontal_length;
                let point = center - horizontal * plane_distance;
                if triangle.contains_point(point) {
                    nearest = Some(RayHit {
                        fraction,
                        point,
                        normal: outward,
                    });
                }
            }
        }
        nearest
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
    /// A low source-surface spawn with a supported board footprint and two
    /// source units of vertical clearance. This remains derived visual geometry,
    /// not proof that the original game marked the surface as skateable.
    pub fn ground_spawn(&self) -> Option<Vec3> {
        let mut candidates: Vec<_> = self
            .triangles
            .iter()
            .filter_map(|t| {
                let area = ((t.b - t.a).cross(t.c - t.a)).y.abs();
                // Half a square source unit of projected area, at most ~26 degrees.
                if t.normal.y.abs() < 0.9 || area < 1.0 {
                    return None;
                }
                let surface = (t.a + t.b + t.c) * (1.0 / 3.0);
                Some((surface, area))
            })
            .collect();
        candidates.sort_by(|(a, area_a), (b, area_b)| {
            a.y.total_cmp(&b.y)
                .then_with(|| area_b.total_cmp(area_a))
                .then_with(|| a.x.total_cmp(&b.x))
                .then_with(|| a.z.total_cmp(&b.z))
        });
        // Test candidates in selection order, stopping at the first safe one.
        candidates.into_iter().find_map(|(surface, _)| {
            if (self.height(surface.x, surface.z, surface.y + 0.2)? - surface.y).abs() > 0.001 {
                return None;
            }
            let board = surface + Vec3::new(0.0, BOARD_HEIGHT, 0.0);
            for (dx, dz) in [(0., 0.), (-0.18, 0.), (0.18, 0.), (0., -0.18), (0., 0.18)] {
                let x = surface.x + dx;
                let z = surface.z + dz;
                let floor = self.height(x, z, surface.y + 0.2)?;
                if (floor - surface.y).abs() > 0.2
                    || self.triangles.iter().any(|other| {
                        other.vertical_height(x, z).is_some_and(|ceiling| {
                            ceiling > floor + 0.0001 && ceiling < floor + 2.0
                        })
                    })
                    || self.blocks_motion(board, Vec3::new(x, board.y, z))
                {
                    return None;
                }
            }
            Some(board)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skating::{FIXED_DT, Input, Skater};
    fn square(x: f32, z: f32, y: f32, half: f32) -> [[Vec3; 3]; 2] {
        let a = Vec3::new(x - half, y, z - half);
        let b = Vec3::new(x + half, y, z - half);
        let c = Vec3::new(x - half, y, z + half);
        let d = Vec3::new(x + half, y, z + half);
        [[a, b, c], [b, d, c]]
    }
    fn wall_x(x: f32) -> [[Vec3; 3]; 2] {
        let a = Vec3::new(x, -1., -5.);
        let b = Vec3::new(x, 4., -5.);
        let c = Vec3::new(x, -1., 5.);
        let d = Vec3::new(x, 4., 5.);
        [[a, b, c], [b, d, c]]
    }
    #[test]
    fn segment_query_returns_nearest_two_sided_contact_and_oriented_unit_normal() {
        let mut faces = wall_x(1.).to_vec();
        faces.extend(wall_x(0.));
        for reverse in [false, true] {
            let faces: Vec<_> = faces
                .iter()
                .map(|&[a, b, c]| if reverse { [c, b, a] } else { [a, b, c] })
                .collect();
            let mesh = TriangleMesh::from_triangles(&faces).unwrap();
            let hit = mesh
                .raycast(Vec3::new(-2., 1., 0.), Vec3::new(2., 1., 0.))
                .unwrap();
            assert!((hit.fraction - 0.5).abs() < 1e-6);
            assert_eq!(hit.point, Vec3::new(0., 1., 0.));
            assert!(hit.normal.x < -0.999 && (hit.normal.dot(hit.normal) - 1.).abs() < 1e-6);
            let reverse = mesh
                .raycast(Vec3::new(2., 1., 0.), Vec3::new(-2., 1., 0.))
                .unwrap();
            assert!((reverse.fraction - 0.25).abs() < 1e-6);
            assert_eq!(reverse.point, Vec3::new(1., 1., 0.));
            assert!(reverse.normal.x > 0.999);
            assert_eq!(
                mesh.raycast(Vec3::new(-1., 1., 0.), Vec3::new(0., 1., 0.))
                    .unwrap()
                    .fraction,
                1.
            );
        }
    }
    #[test]
    fn segment_query_rejects_invalid_parallel_zero_length_and_outside_segments() {
        let mesh = TriangleMesh::from_triangles(&wall_x(0.)).unwrap();
        for (from, to) in [
            (Vec3::new(f32::NAN, 0., 0.), Vec3::default()),
            (Vec3::default(), Vec3::new(1_000_000., 0., 0.)),
            (Vec3::new(-1., 0., 0.), Vec3::new(-1., 2., 0.)),
            (Vec3::new(-1., 0., 6.), Vec3::new(1., 0., 6.)),
            (Vec3::default(), Vec3::default()),
        ] {
            assert!(mesh.raycast(from, to).is_none());
        }
        let tiny = TriangleMesh::from_triangles(&[[
            Vec3::default(),
            Vec3::new(0.001, 0., 0.),
            Vec3::new(0., 0., 0.001),
        ]])
        .unwrap();
        let hit = tiny
            .raycast(
                Vec3::new(0.0002, 1., 0.0002),
                Vec3::new(0.0002, -1., 0.0002),
            )
            .unwrap();
        assert!((hit.normal.dot(hit.normal) - 1.).abs() < 1e-6);
    }
    #[test]
    fn body_motion_keeps_tangential_distance_and_radius_on_both_wall_sides() {
        let mesh = TriangleMesh::from_triangles(&wall_x(0.)).unwrap();
        for side in [-1., 1.] {
            let from = Vec3::new(side, BOARD_HEIGHT, -1.);
            let to = Vec3::new(-side, 0.4, 1.);
            let motion = mesh.resolve_motion(from, to);
            assert!(motion.blocked);
            assert!((motion.position.x - side * BODY_RADIUS).abs() < 1e-5);
            assert!((motion.position.z - to.z).abs() < 1e-5);
            assert_eq!(motion.position.y, to.y);
            let actual = Vec3::new(motion.position.x - from.x, 0., motion.position.z - from.z);
            let requested = Vec3::new(to.x - from.x, 0., to.z - from.z);
            assert!(actual.dot(actual) <= requested.dot(requested) + 1e-5);
        }
    }
    #[test]
    fn body_motion_does_not_stick_at_rest_along_radius_or_while_moving_away() {
        let mesh = TriangleMesh::from_triangles(&wall_x(0.)).unwrap();
        for (from, to) in [
            (
                Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, 0.),
                Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, 0.),
            ),
            (
                Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, -1.),
                Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, 1.),
            ),
            (
                Vec3::new(-0.1, BOARD_HEIGHT, 0.),
                Vec3::new(-0.5, BOARD_HEIGHT, 0.),
            ),
            (
                Vec3::new(0., BOARD_HEIGHT, 0.),
                Vec3::new(0.5, BOARD_HEIGHT, 0.),
            ),
        ] {
            assert_eq!(
                mesh.resolve_motion(from, to),
                Motion {
                    position: to,
                    blocked: false
                }
            );
        }
        let from = Vec3::new(-1., BOARD_HEIGHT, 0.);
        assert_eq!(
            mesh.resolve_motion(from, Vec3::new(f32::NAN, 0., 0.)),
            Motion {
                position: from,
                blocked: true
            }
        );
        let from = Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, 0.);
        let contact = mesh.resolve_motion(from, Vec3::new(1., BOARD_HEIGHT, 1.));
        assert!(contact.blocked);
        assert!((contact.position.x + BODY_RADIUS).abs() < 1e-6);
        assert!((contact.position.z - 1.).abs() < 1e-6);
    }
    #[test]
    fn body_radius_and_slide_are_independent_of_triangle_winding() {
        let faces = wall_x(0.);
        let reversed = faces.map(|[a, b, c]| [c, b, a]);
        let forward = TriangleMesh::from_triangles(&faces).unwrap();
        let backward = TriangleMesh::from_triangles(&reversed).unwrap();
        for (from, to) in [
            (
                Vec3::new(-1., BOARD_HEIGHT, -1.),
                Vec3::new(1., BOARD_HEIGHT, 1.),
            ),
            (
                Vec3::new(1., BOARD_HEIGHT, -1.),
                Vec3::new(-1., BOARD_HEIGHT, 1.),
            ),
            (
                Vec3::new(-BODY_RADIUS, BOARD_HEIGHT, 0.),
                Vec3::new(1., BOARD_HEIGHT, 1.),
            ),
        ] {
            assert_eq!(
                forward.resolve_motion(from, to),
                backward.resolve_motion(from, to)
            );
        }
    }
    #[test]
    fn two_contact_motion_stops_at_corner_without_gaining_distance() {
        let mut faces = wall_x(0.).to_vec();
        faces.extend(wall_x(0.).map(|triangle| triangle.map(|p| Vec3::new(p.z, p.y, p.x))));
        let from = Vec3::new(-1., BOARD_HEIGHT, -1.);
        let to = Vec3::new(1., BOARD_HEIGHT, 1.);
        for reverse in [false, true] {
            if reverse {
                faces.reverse();
            }
            let mesh = TriangleMesh::from_triangles(&faces).unwrap();
            let motion = mesh.resolve_motion(from, to);
            assert!(motion.blocked);
            assert!((motion.position.x + BODY_RADIUS).abs() < 1e-5);
            assert!((motion.position.z + BODY_RADIUS).abs() < 1e-5);
            assert!(
                (motion.position - from).dot(motion.position - from) <= (to - from).dot(to - from)
            );
        }
    }
    #[test]
    fn head_sweep_hits_only_rising_ceiling_crossings_without_depenetration() {
        let mesh = TriangleMesh::from_triangles(&square(0., 0., 2., 3.)).unwrap();
        let from = Vec3::new(0., 0.12, 0.);
        let to = Vec3::new(0., 1., 0.);
        let hit = mesh.ceiling_hit(from, to).unwrap();
        assert!((hit.point.y - 2.).abs() < 1e-6 && hit.normal.y < -0.999);
        assert!((hit.fraction - (2. - BODY_HEAD_HEIGHT - from.y) / (to.y - from.y)).abs() < 1e-6);
        assert!(mesh.ceiling_hit(to, from).is_none());
        assert!(mesh.ceiling_hit(from, from).is_none());
        assert!(mesh.ceiling_hit(Vec3::new(0., 0.6, 0.), to).is_none());
        assert!(mesh.ceiling_hit(Vec3::new(f32::NAN, 0., 0.), to).is_none());
    }
    #[test]
    fn terrain_delegates_collision_and_preserves_default_bounds_behavior() {
        let mut faces = wall_x(0.).to_vec();
        faces.extend(square(0., 0., 2., 3.));
        let mesh = TriangleMesh::from_triangles(&faces).unwrap();
        let from = Vec3::new(-1., BOARD_HEIGHT, -1.);
        let to = Vec3::new(1., BOARD_HEIGHT, 1.);
        let course = World { mesh: Some(&mesh) };
        let source = SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-4., -1., -4.),
            max: Vec3::new(4., 4., 4.),
        };
        for terrain in [&course as &dyn Terrain, &source as &dyn Terrain] {
            assert_eq!(
                terrain.resolve_motion(from, to),
                mesh.resolve_motion(from, to)
            );
            assert!(terrain.raycast(from, to).is_some());
            assert!(
                terrain
                    .ceiling_hit(Vec3::new(-1., 0.12, 0.), Vec3::new(-1., 1., 0.))
                    .is_some()
            );
        }
        assert_eq!(
            source.resolve_motion(from, Vec3::new(-5., BOARD_HEIGHT, 0.)),
            Motion {
                position: from,
                blocked: true
            }
        );
        let flat = World { mesh: None };
        assert_eq!(
            flat.resolve_motion(from, to),
            Motion {
                position: to,
                blocked: false
            }
        );
        assert_eq!(
            flat.resolve_motion(from, Vec3::new(COURSE_X, BOARD_HEIGHT, 0.)),
            Motion {
                position: from,
                blocked: true
            }
        );
        assert!(flat.raycast(from, to).is_none() && flat.ceiling_hit(from, to).is_none());
    }
    #[test]
    fn source_world_preserves_holes_and_support_ceiling_without_course_rails() {
        let mesh = TriangleMesh::from_triangles(&square(0., 0., 3., 2.)).unwrap();
        let world = SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-4., 0., -4.),
            max: Vec3::new(4., 4., 4.),
        };
        assert_eq!(world.support(0., 0., 4.), Some(3.));
        assert_eq!(world.support(0., 0., 2.), None);
        assert_eq!(world.support(3., 0., 4.), None);
        assert_eq!(world.height(3., 0., 4.), f32::NEG_INFINITY);
        assert!(world.rails().is_empty());
        let original = World { mesh: None };
        assert_eq!(
            original.support(0., 10., 100.),
            Some(ground_height(0., 10.))
        );
        assert_eq!(original.rails(), &COURSE_RAILS);
    }
    #[test]
    fn source_bounds_are_inclusive_finite_and_independent_of_course_limits() {
        let mesh = TriangleMesh::from_triangles(&square(100., 100., 0., 3.)).unwrap();
        let world = SourceWorld {
            mesh: &mesh,
            min: Vec3::new(99., 0., 99.),
            max: Vec3::new(101., 0., 101.),
        };
        assert!(world.contains_motion(Vec3::default(), Vec3::new(101., 2., 100.)));
        assert!(!world.contains_motion(Vec3::default(), Vec3::new(101.01, 2., 100.)));
        assert!(!world.contains_motion(Vec3::default(), Vec3::new(100., f32::NAN, 100.)));
        assert_eq!(world.support(102., 100., 2.), None);
        let original = World { mesh: None };
        assert!(original.contains_motion(Vec3::default(), Vec3::new(COURSE_X - 0.5, 0., 0.)));
        assert!(!original.contains_motion(Vec3::default(), Vec3::new(COURSE_X, 0., 0.)));
        for (min, max) in [
            (Vec3::new(f32::NAN, 0., 99.), world.max),
            (Vec3::new(102., 0., 99.), world.max),
        ] {
            let invalid = SourceWorld {
                mesh: &mesh,
                min,
                max,
            };
            assert_eq!(invalid.support(100., 100., 2.), None);
            assert!(!invalid.contains_motion(Vec3::default(), Vec3::new(100., 0., 100.)));
        }
    }
    #[test]
    fn ground_spawn_chooses_low_substantial_surface_before_roofs_and_small_details() {
        let mut faces = square(0., 0., 0., 2.).to_vec();
        faces.extend(square(0., 0., 3., 2.));
        faces.extend(square(10., 0., -2., 0.1));
        let mesh = TriangleMesh::from_triangles(&faces).unwrap();
        let spawn = mesh.ground_spawn().unwrap();
        assert!((spawn.y - BOARD_HEIGHT).abs() < 1e-6);
        assert!(spawn.x.abs() < 2. && spawn.z.abs() < 2.);
        assert!((mesh.placement().unwrap().y - (3. + BOARD_HEIGHT)).abs() < 1e-6);
    }
    #[test]
    fn ground_spawn_requires_headroom_and_supported_board_footprint() {
        let mut faces = square(0., 0., 0., 2.).to_vec();
        faces.extend(square(0., 0., 1.5, 2.));
        faces.extend(square(10., 0., 0., 2.));
        let mesh = TriangleMesh::from_triangles(&faces).unwrap();
        let spawn = mesh.ground_spawn().unwrap();
        assert!((spawn.y - BOARD_HEIGHT).abs() < 1e-6);
        assert!(
            spawn.x > 8.,
            "The low ceiling excludes the first ground patch"
        );
        let narrow = TriangleMesh::from_triangles(&[[
            Vec3::new(-0.05, 0., -20.),
            Vec3::new(0.05, 0., -20.),
            Vec3::new(0., 0., 20.),
        ]])
        .unwrap();
        assert!(narrow.ground_spawn().is_none());
    }
    #[test]
    fn ground_spawn_breaks_equal_height_ties_by_projected_area() {
        let mut faces = square(0., 0., 0., 1.).to_vec();
        faces.extend(square(10., 0., 0., 3.));
        let forward = TriangleMesh::from_triangles(&faces)
            .unwrap()
            .ground_spawn()
            .unwrap();
        faces.reverse();
        let reversed = TriangleMesh::from_triangles(&faces)
            .unwrap()
            .ground_spawn()
            .unwrap();
        assert_eq!(forward, reversed);
        assert!(forward.x > 7.);
    }
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
    #[test]
    fn rising_head_also_contacts_a_steep_sloped_overhang() {
        let a = Vec3::new(-0.05, -3., -2.);
        let b = Vec3::new(0.05, 7., -2.);
        let c = Vec3::new(-0.05, -3., 2.);
        let d = Vec3::new(0.05, 7., 2.);
        for faces in [[[a, b, c], [b, d, c]], [[c, b, a], [c, d, b]]] {
            let mesh = TriangleMesh::from_triangles(&faces).unwrap();
            let hit = mesh
                .ceiling_hit(Vec3::new(0., 0.12, 0.), Vec3::new(0., 1., 0.))
                .unwrap();
            assert!((hit.point.y - 2.).abs() < 1e-5);
            assert!(hit.normal.y < 0. && hit.normal.y > -0.1);
        }
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
