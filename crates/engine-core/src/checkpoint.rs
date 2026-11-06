//! Original in-memory session spots. No game save data is read or written.
use crate::{
    collision::{BODY_HEAD_HEIGHT, Terrain},
    rails::BOARD_HEIGHT,
    skating::{Skater, Vec3},
};

const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
const DIRECTIONS: [[f32; 2]; 8] = [
    [1., 0.],
    [-1., 0.],
    [0., 1.],
    [0., -1.],
    [DIAGONAL, DIAGONAL],
    [-DIAGONAL, DIAGONAL],
    [DIAGONAL, -DIAGONAL],
    [-DIAGONAL, -DIAGONAL],
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    position: Vec3,
    yaw: f32,
    switch_stance: bool,
}
impl Marker {
    pub fn capture(skater: &Skater, terrain: &(impl Terrain + ?Sized)) -> Result<Self, String> {
        if !skater.grounded || skater.grind.is_some() || skater.bail_time != 0. {
            return Err(
                "Set a spot while riding on a supported surface, outside a bail or grind.".into(),
            );
        }
        if !skater.speed.is_finite()
            || skater.speed < 0.
            || !skater.vertical_speed.is_finite()
            || skater.vertical_speed.abs() > 0.05
            || !skater.board_roll.is_finite()
            || skater.board_roll.abs() > 0.01
        {
            return Err("The board must be stable before setting a spot.".into());
        }
        validate_pose(skater.position, skater.yaw, terrain)?;
        Ok(Self {
            position: skater.position,
            yaw: skater.yaw,
            switch_stance: skater.switch_stance,
        })
    }
    /// Recheck the active terrain before changing any rider state. The caller
    /// owns mode/source identity and discards markers when that identity changes.
    pub fn restore(
        &self,
        skater: &mut Skater,
        terrain: &(impl Terrain + ?Sized),
    ) -> Result<(), String> {
        validate_pose(self.position, self.yaw, terrain)?;
        skater.recover_at(self.position, self.yaw)?;
        skater.switch_stance = self.switch_stance;
        skater.last_trick = "Returned to session spot";
        Ok(())
    }
    pub fn position(&self) -> Vec3 {
        self.position
    }
    /// Captured body yaw; switch stance is preserved separately.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    pub fn switch_stance(&self) -> bool {
        self.switch_stance
    }
    /// Captured travel direction for the marker arrow.
    pub fn heading(&self) -> Vec3 {
        let yaw = self.yaw
            + if self.switch_stance {
                std::f32::consts::PI
            } else {
                0.
            };
        Vec3::new(yaw.sin(), 0., yaw.cos())
    }
}

fn validate_pose(
    position: Vec3,
    yaw: f32,
    terrain: &(impl Terrain + ?Sized),
) -> Result<(), String> {
    if ![position.x, position.y, position.z, yaw]
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1_000_000.)
        || !terrain.contains_motion(position, position)
    {
        return Err("The spot is outside valid terrain bounds.".into());
    }
    let surface = terrain
        .support(position.x, position.z, position.y + 0.08)
        .filter(|height| height.is_finite())
        .ok_or("The spot has no supported riding surface.")?;
    if (position.y - surface - BOARD_HEIGHT).abs() > 0.02 {
        return Err("The spot is not resting on its riding surface.".into());
    }
    // Center and eight surrounding support/headroom samples form a small,
    // conservative footprint. These are bounded probes, not a capsule solver.
    for [dx, dz] in std::iter::once([0., 0.]).chain(DIRECTIONS) {
        let x = position.x + dx * 0.18;
        let z = position.z + dz * 0.18;
        let height = terrain
            .support(x, z, position.y + 0.08)
            .filter(|height| height.is_finite())
            .ok_or("The spot is too close to a gap or surface edge.")?;
        let board = Vec3::new(x, height + BOARD_HEIGHT, z);
        if (height - surface).abs() > 0.12 || !terrain.contains_motion(position, board) {
            return Err("The spot needs a supported footprint inside the terrain bounds.".into());
        }
        if terrain
            .raycast(
                board + Vec3::new(0., 0.02, 0.),
                board + Vec3::new(0., BODY_HEAD_HEIGHT, 0.),
            )
            .is_some()
        {
            return Err("The spot has insufficient body or head clearance.".into());
        }
    }
    for [dx, dz] in DIRECTIONS {
        let step = position + Vec3::new(dx * 0.06, 0., dz * 0.06);
        let motion = terrain.resolve_motion(position, step);
        let difference = motion.position - step;
        if !terrain.contains_motion(position, step)
            || terrain.blocks_motion(position, step)
            || motion.blocked
            || ![difference.x, difference.y, difference.z]
                .iter()
                .all(|v| v.is_finite())
            || difference.dot(difference) > 0.000001
        {
            return Err("The spot is too close to a wall or blocked movement.".into());
        }
        // Ray starts also reject a center exactly on a two-sided wall plane,
        // where a movement resolver deliberately permits escape either way.
        for height in [0.25, 0.9, 1.5] {
            let from = position + Vec3::new(0., height - BOARD_HEIGHT, 0.);
            if terrain
                .raycast(from, from + Vec3::new(dx * 0.2, 0., dz * 0.2))
                .is_some()
            {
                return Err("The spot overlaps nearby body geometry.".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{SourceWorld, TriangleMesh, World};
    fn floor(y: f32) -> Vec<[Vec3; 3]> {
        let a = Vec3::new(-2., y, -2.);
        let b = Vec3::new(2., y, -2.);
        let c = Vec3::new(-2., y, 2.);
        let d = Vec3::new(2., y, 2.);
        vec![[a, b, c], [b, d, c]]
    }
    fn world(mesh: &TriangleMesh) -> SourceWorld<'_> {
        SourceWorld {
            mesh,
            min: Vec3::new(-2., 0., -2.),
            max: Vec3::new(2., 3., 2.),
        }
    }
    fn rider() -> Skater {
        Skater::at_spawn(Vec3::new(0., 0.12, 0.), 0.4).unwrap()
    }
    #[test]
    fn capture_is_immutable_and_restore_keeps_banked_stats_and_switch_pose() {
        let mesh = TriangleMesh::from_triangles(&floor(0.)).unwrap();
        for switch in [false, true] {
            let mut skater = rider();
            skater.switch_stance = switch;
            skater.speed = 4.;
            skater.score = 99;
            skater.combo.add(400);
            let before = format!("{skater:?}");
            let marker = Marker::capture(&skater, &world(&mesh)).unwrap();
            assert_eq!(format!("{skater:?}"), before);
            assert_eq!(marker.position(), skater.position);
            assert_eq!(marker.yaw(), skater.yaw);
            assert_eq!(marker.switch_stance(), switch);
            skater.position = Vec3::new(1., 3., 1.);
            skater.grounded = false;
            skater.speed = 7.;
            skater.vertical_speed = -3.;
            skater.board_roll = 2.;
            skater.bail_time = 0.5;
            skater.score = 1234;
            skater.landings = 7;
            skater.last_airtime = 1.25;
            skater.last_combo = 600;
            marker.restore(&mut skater, &world(&mesh)).unwrap();
            assert_eq!(skater.position, marker.position());
            assert_eq!(skater.yaw, marker.yaw());
            assert_eq!(skater.switch_stance, switch);
            assert!(
                (skater.travel_direction() - marker.heading())
                    .dot(skater.travel_direction() - marker.heading())
                    < 1e-10
            );
            assert_eq!(
                (skater.score, skater.landings, skater.last_combo),
                (1234, 7, 600)
            );
            assert_eq!(skater.last_airtime, 1.25);
            assert!(
                skater.grounded
                    && skater.speed == 0.
                    && skater.vertical_speed == 0.
                    && skater.board_roll == 0.
                    && skater.bail_time == 0.
                    && skater.grind.is_none()
            );
            assert_eq!(skater.combo.value(), 0);
        }
    }
    #[test]
    fn capture_rejects_air_bails_grinds_and_invalid_or_unstable_pose() {
        let terrain = World { mesh: None };
        let mut bad = Vec::new();
        let mut s = rider();
        s.grounded = false;
        bad.push(s);
        let mut s = rider();
        s.bail_time = 0.1;
        bad.push(s);
        let mut s = rider();
        s.grind = Some(crate::rails::Grind {
            rail_index: 0,
            balance: 0.,
            distance: 0.,
            time: 0.,
            rail: crate::rails::COURSE_RAILS[0],
            along: 0.,
            direction: 1.,
        });
        bad.push(s);
        let mut s = rider();
        s.position.x = f32::NAN;
        bad.push(s);
        let mut s = rider();
        s.yaw = f32::INFINITY;
        bad.push(s);
        let mut s = rider();
        s.position.y = 0.5;
        bad.push(s);
        let mut s = rider();
        s.vertical_speed = 1.;
        bad.push(s);
        let mut s = rider();
        s.board_roll = 1.;
        bad.push(s);
        for s in bad {
            assert!(Marker::capture(&s, &terrain).is_err());
        }
    }
    #[test]
    fn capture_rejects_edges_low_roofs_walls_and_exact_wall_plane_contacts() {
        let mesh = TriangleMesh::from_triangles(&floor(0.)).unwrap();
        let mut edge = rider();
        edge.position.x = 1.9;
        assert!(Marker::capture(&edge, &world(&mesh)).is_err());
        let mut faces = floor(0.);
        faces.extend(floor(1.5));
        let roof = TriangleMesh::from_triangles(&faces).unwrap();
        assert!(Marker::capture(&rider(), &world(&roof)).is_err());
        for x in [0., 0.19] {
            let mut faces = floor(0.);
            faces.extend([
                [
                    Vec3::new(x, 0., -2.),
                    Vec3::new(x, 3., -2.),
                    Vec3::new(x, 0., 2.),
                ],
                [
                    Vec3::new(x, 3., -2.),
                    Vec3::new(x, 3., 2.),
                    Vec3::new(x, 0., 2.),
                ],
            ]);
            let wall = TriangleMesh::from_triangles(&faces).unwrap();
            assert!(Marker::capture(&rider(), &world(&wall)).is_err());
        }
    }
    #[test]
    fn restore_revalidates_terrain_and_leaves_rider_unchanged_when_unsafe() {
        let clear = TriangleMesh::from_triangles(&floor(0.)).unwrap();
        let marker = Marker::capture(&rider(), &world(&clear)).unwrap();
        let mut faces = floor(0.);
        faces.extend(floor(1.5));
        let roof = TriangleMesh::from_triangles(&faces).unwrap();
        let moved = TriangleMesh::from_triangles(&floor(1.)).unwrap();
        let mut skater = rider();
        skater.position = Vec3::new(1., 2., 1.);
        skater.score = 987;
        skater.combo.add(400);
        for unsafe_mesh in [&roof, &moved] {
            let before = format!("{skater:?}");
            assert!(marker.restore(&mut skater, &world(unsafe_mesh)).is_err());
            assert_eq!(format!("{skater:?}"), before);
        }
    }
    #[test]
    fn default_terrain_motion_checks_and_course_bounds_are_honored() {
        struct Trapped;
        impl Terrain for Trapped {
            fn height(&self, _: f32, _: f32, _: f32) -> f32 {
                0.
            }
            fn blocks_motion(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
        }
        assert!(Marker::capture(&rider(), &Trapped).is_err());
        let mut edge = rider();
        edge.position.x = crate::skating::COURSE_X - 0.55;
        assert!(Marker::capture(&edge, &World { mesh: None }).is_err());
        assert!(Marker::capture(&rider(), &World { mesh: None }).is_ok());
    }
}
