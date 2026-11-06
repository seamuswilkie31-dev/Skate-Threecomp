//! Original render-only slope alignment for the placeholder board and rider.
use threecomp_engine_core::{
    collision::Terrain,
    rails::BOARD_HEIGHT,
    skating::{Skater, Vec3},
};

const SAMPLE_RADIUS: f32 = 0.15;
const MAX_SAMPLE_RISE: f32 = 0.2;
const MIN_UP: f32 = 0.64;

#[derive(Debug, Clone, Copy)]
pub struct Basis {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}
impl Basis {
    /// Matches the prototype's upright yaw rotation, with a finite fallback.
    pub fn upright(yaw: f32) -> Self {
        let yaw = if yaw.is_finite() { yaw } else { 0. };
        Self {
            right: Vec3::new(yaw.cos(), 0., -yaw.sin()),
            up: Vec3::new(0., 1., 0.),
            forward: Vec3::new(yaw.sin(), 0., yaw.cos()),
        }
    }
    pub fn transform(self, local: Vec3) -> Vec3 {
        self.right * local.x + self.up * local.y + self.forward * local.z
    }
}

/// Infer a small local height gradient without changing physics. The support
/// ceiling and rise limits prevent a separate upper floor from becoming a pose
/// target. Missing samples, air, grinds, and invalid poses retain upright yaw.
pub fn basis(skater: &Skater, terrain: &(impl Terrain + ?Sized)) -> Basis {
    let upright = Basis::upright(skater.yaw);
    if !skater.grounded
        || skater.grind.is_some()
        || !skater.yaw.is_finite()
        || !finite(skater.position)
    {
        return upright;
    }
    let position = skater.position;
    let ceiling = position.y + 0.08;
    let Some(center) = terrain.support(position.x, position.z, ceiling) else {
        return upright;
    };
    if !center.is_finite() || (position.y - center - BOARD_HEIGHT).abs() > 0.025 {
        return upright;
    }
    let sample = |dx, dz| {
        let point = Vec3::new(position.x + dx, position.y, position.z + dz);
        if !terrain.contains_motion(position, point) {
            return None;
        }
        terrain
            .support(point.x, point.z, ceiling)
            .filter(|height| height.is_finite() && (*height - center).abs() <= MAX_SAMPLE_RISE)
    };
    let (Some(left), Some(right), Some(back), Some(front)) = (
        sample(-SAMPLE_RADIUS, 0.),
        sample(SAMPLE_RADIUS, 0.),
        sample(0., -SAMPLE_RADIUS),
        sample(0., SAMPLE_RADIUS),
    ) else {
        return upright;
    };
    let mut gx = (right - left) / (2. * SAMPLE_RADIUS);
    let mut gz = (front - back) / (2. * SAMPLE_RADIUS);
    let gradient_length = (gx * gx + gz * gz).sqrt();
    if gradient_length < 1e-5 {
        return upright;
    }
    let max_gradient = (1. / (MIN_UP * MIN_UP) - 1.).sqrt();
    if gradient_length > max_gradient {
        let scale = max_gradient / gradient_length;
        gx *= scale;
        gz *= scale;
    }
    let up = Vec3::new(-gx, 1., -gz).normalized();
    let forward = Vec3::new(
        upright.forward.x,
        gx * upright.forward.x + gz * upright.forward.z,
        upright.forward.z,
    )
    .normalized();
    let right = up.cross(forward).normalized();
    Basis { right, up, forward }
}

fn finite(point: Vec3) -> bool {
    [point.x, point.y, point.z]
        .iter()
        .all(|value| value.is_finite() && value.abs() < 1_000_000.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use threecomp_engine_core::collision::{SourceWorld, TriangleMesh};

    struct Plane {
        gx: f32,
        gz: f32,
    }
    impl Terrain for Plane {
        fn height(&self, x: f32, z: f32, _ceiling: f32) -> f32 {
            self.gx * x + self.gz * z
        }
        fn support(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
            let height = self.height(x, z, ceiling);
            (height <= ceiling).then_some(height)
        }
        fn blocks_motion(&self, _from: Vec3, _to: Vec3) -> bool {
            false
        }
    }
    fn close(a: Vec3, b: Vec3) {
        let delta = a - b;
        assert!(delta.dot(delta) < 1e-9, "{a:?} != {b:?}");
    }
    fn valid_basis(pose: Basis) {
        for axis in [pose.right, pose.up, pose.forward] {
            assert!((axis.dot(axis) - 1.).abs() < 1e-5);
            assert!(finite(axis));
        }
        assert!(pose.right.dot(pose.up).abs() < 1e-5);
        assert!(pose.right.dot(pose.forward).abs() < 1e-5);
        assert!(pose.up.dot(pose.forward).abs() < 1e-5);
        close(pose.right.cross(pose.up), pose.forward);
        assert!(pose.up.y >= MIN_UP - 1e-5);
    }
    fn skater(yaw: f32) -> Skater {
        Skater::at_spawn(Vec3::new(0., BOARD_HEIGHT, 0.), yaw).unwrap()
    }

    #[test]
    fn flat_pose_preserves_upright_yaw_and_local_offsets() {
        let local = Vec3::new(0.2, 0.7, -0.4);
        for yaw in [0., 0.5, -1.5, std::f32::consts::PI] {
            let pose = basis(&skater(yaw), &Plane { gx: 0., gz: 0. });
            close(
                pose.transform(local),
                Vec3::new(
                    local.x * yaw.cos() + local.z * yaw.sin(),
                    local.y,
                    -local.x * yaw.sin() + local.z * yaw.cos(),
                ),
            );
            assert_eq!(pose.up, Vec3::new(0., 1., 0.));
            valid_basis(pose);
        }
    }

    #[test]
    fn grade_and_cross_slope_form_bounded_right_handed_basis_at_any_yaw() {
        for (gx, gz) in [(0., 0.5), (0.5, 0.), (-0.4, 0.7), (0.9, 0.9)] {
            let plane = Plane { gx, gz };
            for yaw in [0., 0.7, std::f32::consts::FRAC_PI_2, std::f32::consts::PI] {
                let pose = basis(&skater(yaw), &plane);
                valid_basis(pose);
                let max_gradient = (1. / (MIN_UP * MIN_UP) - 1.).sqrt();
                let scale = (max_gradient / (gx * gx + gz * gz).sqrt()).min(1.);
                close(
                    pose.up,
                    Vec3::new(-gx * scale, 1., -gz * scale).normalized(),
                );
                let horizontal_yaw = pose.forward.x.atan2(pose.forward.z);
                assert!((horizontal_yaw.sin() - yaw.sin()).abs() < 1e-5);
                assert!((horizontal_yaw.cos() - yaw.cos()).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn overhead_floor_is_excluded_by_support_ceiling() {
        let mut faces = Vec::new();
        for offset in [0., 1.5] {
            faces.extend([
                [
                    Vec3::new(-2., -0.5 + offset, -2.),
                    Vec3::new(2., -0.5 + offset, -2.),
                    Vec3::new(-2., 0.5 + offset, 2.),
                ],
                [
                    Vec3::new(2., -0.5 + offset, -2.),
                    Vec3::new(2., 0.5 + offset, 2.),
                    Vec3::new(-2., 0.5 + offset, 2.),
                ],
            ]);
        }
        let mesh = TriangleMesh::from_triangles(&faces).unwrap();
        let world = SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., -0.5, -2.),
            max: Vec3::new(2., 2., 2.),
        };
        let pose = basis(&skater(0.), &world);
        close(pose.up, Vec3::new(0., 1., -0.25).normalized());
        valid_basis(pose);
    }

    #[test]
    fn missing_support_air_and_invalid_poses_fall_back_upright() {
        let mut rider = skater(0.8);
        let upright = Basis::upright(rider.yaw);
        let plane = Plane { gx: 0.4, gz: 0.2 };
        rider.grounded = false;
        close(basis(&rider, &plane).up, upright.up);
        rider.grounded = true;
        rider.position.y += 0.5;
        close(basis(&rider, &plane).up, upright.up);
        rider.position.x = f32::NAN;
        close(basis(&rider, &plane).up, upright.up);
        rider.position = Vec3::new(19.45, BOARD_HEIGHT, 0.);
        close(basis(&rider, &Plane { gx: 0., gz: 0.2 }).up, upright.up);
        close(
            basis(
                &skater(0.8),
                &Plane {
                    gx: f32::NAN,
                    gz: 0.,
                },
            )
            .up,
            upright.up,
        );
        let mut rider = skater(0.8);
        rider.yaw = f32::NAN;
        close(basis(&rider, &plane).forward, Basis::upright(0.).forward);
    }

    #[test]
    fn unsupported_footprint_edge_keeps_pose_upright() {
        let mesh = TriangleMesh::from_triangles(&[
            [
                Vec3::new(-2., -0.5, -2.),
                Vec3::new(0.1, -0.5, -2.),
                Vec3::new(-2., 0.5, 2.),
            ],
            [
                Vec3::new(0.1, -0.5, -2.),
                Vec3::new(0.1, 0.5, 2.),
                Vec3::new(-2., 0.5, 2.),
            ],
        ])
        .unwrap();
        let world = SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., -0.5, -2.),
            max: Vec3::new(2., 0.5, 2.),
        };
        let rider = skater(0.4);
        assert!(world.support(0., 0., 0.2).is_some());
        assert!(world.support(SAMPLE_RADIUS, 0., 0.2).is_none());
        close(basis(&rider, &world).up, Vec3::new(0., 1., 0.));
    }
}
