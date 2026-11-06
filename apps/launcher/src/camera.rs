//! Original third-person follow/orbit camera with bounded clearance probes.
use threecomp_engine_core::{collision::Terrain, skating::Vec3};

const DEFAULT_DISTANCE: f32 = 7.5;
const DEFAULT_PITCH: f32 = 0.33;

pub struct FollowCamera {
    pub eye: Vec3,
    pub target: Vec3,
    yaw_offset: f32,
    pitch: f32,
    distance: f32,
}
impl FollowCamera {
    pub fn new(position: Vec3, heading: Vec3) -> Self {
        let mut camera = Self {
            eye: Vec3::default(),
            target: Vec3::default(),
            yaw_offset: 0.,
            pitch: DEFAULT_PITCH,
            distance: DEFAULT_DISTANCE,
        };
        camera.reset(position, heading);
        camera
    }
    pub fn reset(&mut self, position: Vec3, heading: Vec3) {
        self.yaw_offset = 0.;
        self.pitch = DEFAULT_PITCH;
        self.distance = DEFAULT_DISTANCE;
        let (eye, target) = self.desired(position, heading);
        self.eye = eye;
        self.target = target;
    }
    fn desired(&self, position: Vec3, heading: Vec3) -> (Vec3, Vec3) {
        let yaw = heading.x.atan2(heading.z) + self.yaw_offset;
        let backward = Vec3::new(-yaw.sin(), 0., -yaw.cos());
        let pivot = position + Vec3::new(0., 1.1, 0.);
        let eye = pivot
            + backward * (self.distance * self.pitch.cos())
            + Vec3::new(0., self.distance * self.pitch.sin(), 0.);
        (eye, pivot + heading * 1.5)
    }
    /// Orbit is radians per frame; zoom is wheel delta. Camera movement is view
    /// state only and cannot change the rider's travel or trick rotation.
    pub fn update(
        &mut self,
        position: Vec3,
        heading: Vec3,
        input: CameraInput,
        dt: f32,
        aspect: f32,
        terrain: &(impl Terrain + ?Sized),
    ) {
        if !finite(position) || !finite(heading) || !dt.is_finite() || dt <= 0. {
            return;
        }
        let heading = Vec3::new(heading.x, 0., heading.z).normalized();
        if input.recenter {
            self.yaw_offset = 0.;
            self.pitch = DEFAULT_PITCH;
            self.distance = DEFAULT_DISTANCE;
        }
        let bounded = |v: f32| {
            if v.is_finite() {
                v.clamp(-0.5, 0.5)
            } else {
                0.
            }
        };
        self.yaw_offset = (self.yaw_offset + bounded(input.orbit[0]) + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.pitch = (self.pitch + bounded(input.orbit[1])).clamp(0.08, 1.15);
        if input.zoom.is_finite() {
            self.distance =
                (self.distance * (-input.zoom.clamp(-200., 200.) * 0.005).exp()).clamp(2., 12.);
        }
        let (desired, target) = self.desired(position, heading);
        let blend = 1. - (-8. * dt.min(0.1)).exp();
        let candidate = self.eye + (desired - self.eye) * blend;
        let pivot = position + Vec3::new(0., 1.1, 0.);
        // Approximate the near-plane footprint with a center ray and four
        // parallel offset rays. This is not an exact swept-sphere solver.
        let radius = clearance_radius(aspect);
        self.eye = clearance(pivot, candidate, radius, terrain);
        if let Some(floor) = terrain.support(self.eye.x, self.eye.z, self.eye.y + 0.2)
            && floor.is_finite()
            && self.eye.y < floor + 0.2
        {
            // The procedural course has no triangle raycast; keep its camera
            // above the height field, then recheck the anchor-to-eye line.
            let lifted = Vec3::new(self.eye.x, floor + 0.2, self.eye.z);
            self.eye = clearance(pivot, lifted, radius, terrain);
        }
        self.target = self.target + (target - self.target) * blend;
    }
}
#[derive(Default)]
pub struct CameraInput {
    pub orbit: [f32; 2],
    pub zoom: f32,
    pub recenter: bool,
}
fn finite(p: Vec3) -> bool {
    [p.x, p.y, p.z].iter().all(|v| v.is_finite())
}
fn clearance_radius(aspect: f32) -> f32 {
    let aspect = if aspect.is_finite() {
        aspect.clamp(0.25, 6.)
    } else {
        1.
    };
    let half_width = 0.15 * aspect.max(1.) / 1.8;
    let half_height = 0.15 * (1. / aspect).max(1.) / 1.8;
    (half_width * half_width + half_height * half_height)
        .sqrt()
        .clamp(0.14, 0.5)
        + 0.04
}
fn clearance(anchor: Vec3, eye: Vec3, radius: f32, terrain: &(impl Terrain + ?Sized)) -> Vec3 {
    let delta = eye - anchor;
    let distance = delta.dot(delta).sqrt();
    if distance < 0.0001 {
        return anchor;
    }
    let direction = delta * (1. / distance);
    let mut right = Vec3::new(0., 1., 0.).cross(direction).normalized();
    if right.dot(right) < 0.5 {
        right = Vec3::new(1., 0., 0.);
    }
    let up = direction.cross(right).normalized();
    let mut fraction = 1.0f32;
    for offset in [
        Vec3::default(),
        right * radius,
        right * (-radius),
        up * radius,
        up * (-radius),
    ] {
        if let Some(hit) = terrain.raycast(anchor + offset, eye + offset) {
            fraction = fraction.min((hit.fraction - 0.06 / distance).clamp(0., 1.));
        }
    }
    anchor + delta * fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use threecomp_engine_core::collision::{TriangleMesh, World};
    fn wall(x0: f32, x1: f32) -> TriangleMesh {
        TriangleMesh::from_triangles(&[
            [
                Vec3::new(x0, 0., -2.),
                Vec3::new(x1, 0., -2.),
                Vec3::new(x0, 5., -2.),
            ],
            [
                Vec3::new(x1, 0., -2.),
                Vec3::new(x1, 5., -2.),
                Vec3::new(x0, 5., -2.),
            ],
        ])
        .unwrap()
    }
    #[test]
    fn obstacle_pulls_camera_in_immediately_and_clearance_recovers_smoothly() {
        let position = Vec3::new(0., 0.12, 0.);
        let heading = Vec3::new(0., 0., 1.);
        let mesh = wall(-10., 10.);
        let world = World { mesh: Some(&mesh) };
        let mut camera = FollowCamera::new(position, heading);
        camera.update(
            position,
            heading,
            CameraInput::default(),
            1. / 60.,
            1.7,
            &world,
        );
        assert!(camera.eye.z > -2. && camera.eye.z < -1.0);
        let blocked = camera.eye;
        camera.update(
            position,
            heading,
            CameraInput::default(),
            1. / 60.,
            1.7,
            &World { mesh: None },
        );
        assert!(camera.eye.z < blocked.z && camera.eye.z > -4.);
        for _ in 0..120 {
            camera.update(
                position,
                heading,
                CameraInput::default(),
                1. / 60.,
                1.7,
                &World { mesh: None },
            );
        }
        assert!(camera.eye.z < -7.);
    }
    #[test]
    fn offset_probes_catch_edge_that_center_ray_misses() {
        let mesh = wall(0.18, 0.32);
        let world = World { mesh: Some(&mesh) };
        let position = Vec3::new(0., 0.12, 0.);
        let heading = Vec3::new(0., 0., 1.);
        let mut camera = FollowCamera::new(position, heading);
        assert!(
            world
                .raycast(position + Vec3::new(0., 1.1, 0.), camera.eye)
                .is_none()
        );
        camera.update(
            position,
            heading,
            CameraInput::default(),
            1. / 60.,
            1.7,
            &world,
        );
        assert!(camera.eye.z > -2.);
    }
    #[test]
    fn orbit_zoom_limits_recenter_and_invalid_input_are_stable() {
        let position = Vec3::new(0., 0.12, 0.);
        let heading = Vec3::new(0., 0., 1.);
        let world = World { mesh: None };
        let mut camera = FollowCamera::new(position, heading);
        for _ in 0..100 {
            camera.update(
                position,
                heading,
                CameraInput {
                    orbit: [10., 10.],
                    zoom: 200.,
                    recenter: false,
                },
                0.016,
                1.7,
                &world,
            );
        }
        assert_eq!(camera.distance, 2.);
        assert_eq!(camera.pitch, 1.15);
        assert!(finite(camera.eye));
        camera.update(
            position,
            heading,
            CameraInput {
                recenter: true,
                ..Default::default()
            },
            0.016,
            1.7,
            &world,
        );
        assert_eq!(camera.distance, DEFAULT_DISTANCE);
        assert_eq!(camera.pitch, DEFAULT_PITCH);
        assert_eq!(camera.yaw_offset, 0.);
        let eye = camera.eye;
        camera.update(
            position,
            heading,
            CameraInput::default(),
            f32::NAN,
            1.7,
            &world,
        );
        assert_eq!(camera.eye, eye);
        assert!((clearance_radius(2.) - clearance_radius(0.5)).abs() < 1e-6);
        assert!(clearance_radius(0.25) > 0.3);
    }
}
