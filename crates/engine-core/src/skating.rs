//! Original skating movement for a synthetic course, independent of game assets.

use crate::{
    rails::{self, Grind, Rail},
    tricks::{BANK_DELAY, Combo},
};
use std::ops::{Add, Mul, Sub};

pub const FIXED_DT: f32 = 1.0 / 120.0;
pub const COURSE_X: f32 = 20.0;
pub const COURSE_Z: f32 = 40.0;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }
    pub fn normalized(self) -> Self {
        self * (1.0 / self.dot(self).sqrt().max(0.00001))
    }
}
impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Input {
    pub push: bool,
    pub brake: bool,
    pub steer: f32,
    pub ollie: bool,
    pub flip: bool,
    pub spin: f32,
    pub grind: bool,
}

#[derive(Debug, Clone)]
pub struct Skater {
    /// Position of the board above the riding surface, in prototype metres.
    pub position: Vec3,
    pub yaw: f32,
    pub speed: f32,
    pub vertical_speed: f32,
    pub grounded: bool,
    pub landings: u32,
    pub airtime: f32,
    pub last_airtime: f32,
    pub score: u32,
    pub combo: Combo,
    pub last_combo: u32,
    pub grind: Option<Grind>,
    grind_cooldown: f32,
    pub last_trick: &'static str,
    /// Displayed board roll during an active flip.
    pub board_roll: f32,
    flip_time: Option<f32>,
    completed_flips: u32,
    air_spin: f32,
    travel_yaw: f32,
    pub bail_time: f32,
    pub switch_stance: bool,
    air_was_ollie: bool,
}
impl Default for Skater {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.12, -12.0),
            yaw: 0.0,
            speed: 0.0,
            vertical_speed: 0.0,
            grounded: true,
            landings: 0,
            airtime: 0.0,
            last_airtime: 0.0,
            score: 0,
            combo: Combo::default(),
            last_combo: 0,
            grind: None,
            grind_cooldown: 0.0,
            last_trick: "Ready",
            board_roll: 0.0,
            flip_time: None,
            completed_flips: 0,
            air_spin: 0.0,
            travel_yaw: 0.0,
            bail_time: 0.0,
            switch_stance: false,
            air_was_ollie: false,
        }
    }
}
impl Skater {
    pub fn at_spawn(position: Vec3, yaw: f32) -> Result<Self, String> {
        let mut skater = Self::default();
        skater.recover_at(position, yaw)?;
        skater.last_trick = "Ready";
        Ok(skater)
    }
    /// Clear transient movement/trick state after leaving loaded geometry while
    /// preserving already banked points and completed-contact statistics.
    pub fn recover_at(&mut self, position: Vec3, yaw: f32) -> Result<(), String> {
        if ![position.x, position.y, position.z, yaw]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
        {
            return Err("Invalid skating checkpoint.".into());
        }
        let fresh = Self {
            position,
            yaw,
            travel_yaw: yaw,
            score: self.score,
            last_combo: self.last_combo,
            landings: self.landings,
            last_airtime: self.last_airtime,
            last_trick: "Recovered to source start",
            ..Self::default()
        };
        *self = fresh;
        Ok(())
    }
    pub fn travel_direction(&self) -> Vec3 {
        let heading = if self.grounded {
            self.yaw
                + if self.switch_stance {
                    std::f32::consts::PI
                } else {
                    0.0
                }
        } else {
            self.travel_yaw
        };
        Vec3::new(heading.sin(), 0.0, heading.cos())
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
    /// Called at a fixed 120 Hz by the playground. Invalid or large caller steps
    /// are ignored so a paused window cannot destabilize collision integration.
    pub fn step(&mut self, input: Input, dt: f32) {
        self.step_on(input, dt, &crate::collision::World { mesh: None });
    }
    pub fn step_on(
        &mut self,
        input: Input,
        dt: f32,
        terrain: &(impl crate::collision::Terrain + ?Sized),
    ) {
        self.step_with_rails(input, dt, terrain, terrain.rails());
    }
    pub fn step_with_rails(
        &mut self,
        input: Input,
        dt: f32,
        terrain: &(impl crate::collision::Terrain + ?Sized),
        rails: &[Rail],
    ) {
        if !dt.is_finite() || dt <= 0.0 || dt > 1.0 / 30.0 {
            return;
        }
        self.grind_cooldown = (self.grind_cooldown - dt).max(0.0);
        let support_ceiling = self.position.y + 0.08;
        let previous_surface = terrain.support(self.position.x, self.position.z, support_ceiling);
        if self.grounded
            && previous_surface
                .is_none_or(|height| (self.position.y - height - rails::BOARD_HEIGHT).abs() > 0.2)
        {
            self.grounded = false;
            self.vertical_speed = 0.0;
            self.start_air(false);
        }
        if self.bail_time > 0.0 {
            self.bail_time = (self.bail_time - dt).max(0.0);
            self.speed = 0.0;
            // A rail bail falls to the floor during recovery instead of hanging
            // motionless at rail height. Grounded bails remain on their surface.
            let floor = previous_surface.map(|height| height + rails::BOARD_HEIGHT);
            if !self.grounded {
                self.vertical_speed -= 16.0 * dt;
                self.position.y += self.vertical_speed * dt;
            }
            if let Some(floor) = floor
                && (self.position.y <= floor || self.grounded)
            {
                self.position.y = floor;
                self.vertical_speed = 0.0;
                self.grounded = true;
            }
            return;
        }
        if self.grounded && !input.ollie {
            let banked = self.combo.ride(dt);
            if banked > 0 {
                self.score = self.score.saturating_add(banked);
                self.last_combo = banked;
            }
        }
        let steer = if input.steer.is_finite() {
            input.steer.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        if let Some(mut grind) = self.grind.take() {
            // Pop takes precedence when the hold button is released this tick.
            if input.ollie || !input.grind {
                self.exit_grind(grind, input.ollie);
            } else {
                let (axis, length) = grind.rail.axis().expect("Captured rail was validated");
                self.speed = (self.speed - (if input.brake { 9.0 } else { 0.6 }) * dt).max(0.0);
                let previous = grind.along;
                grind.along = (grind.along + grind.direction * self.speed * dt).clamp(0.0, length);
                let next = grind.rail.board_position(grind.along).unwrap();
                if !terrain.contains_motion(self.position, next)
                    || terrain.blocks_motion(self.position, next)
                {
                    self.bail("Bail · rail path blocked");
                    return;
                }
                grind.distance += (grind.along - previous).abs();
                grind.time += dt;
                grind.balance += (steer * 1.4 + 0.18 + 0.12 * (grind.time * 2.4).sin()) * dt;
                self.position = next;
                let heading = axis * grind.direction;
                self.travel_yaw = heading.x.atan2(heading.z);
                if grind.balance.abs() >= 1.0 {
                    self.bail("Bail · grind balance lost");
                } else if grind.along <= 0.0 || grind.along >= length || self.speed < 0.75 {
                    self.exit_grind(grind, false);
                } else {
                    self.grind = Some(grind);
                }
                return;
            }
        }
        let was_grounded = self.grounded;
        let previous_position = self.position;
        if self.grounded {
            self.yaw += steer * (0.6 + self.speed * 0.09) * dt;
        } else {
            let spin = if input.spin.is_finite() {
                input.spin.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let rotation = spin * std::f32::consts::TAU * dt;
            self.yaw += rotation;
            self.air_spin += rotation;
        }
        self.yaw = (self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        if self.grounded {
            if input.push {
                self.speed += 4.0 * dt;
            }
            self.speed =
                (self.speed - (if input.brake { 9.0 } else { 0.35 }) * dt).clamp(0.0, 12.0);
            if input.ollie {
                self.vertical_speed = 6.0;
                self.grounded = false;
                self.airtime = 0.0;
                self.start_air(true);
            }
        }
        let heading = if self.grounded {
            self.yaw
                + if self.switch_stance {
                    std::f32::consts::PI
                } else {
                    0.0
                }
        } else {
            self.travel_yaw
        };
        let requested = Vec3::new(heading.sin(), 0.0, heading.cos()) * (self.speed * dt);
        let mut motion = terrain.resolve_motion(self.position, self.position + requested);
        if !self.grounded && worsens_body_overlap(terrain, previous_position, motion.position) {
            // Head sweeps handle upward travel separately. Reject side entry
            // into an overhang before gravity or landing can leave us inside it.
            motion.position = previous_position;
            motion.blocked = true;
        }
        let movement = Vec3::new(
            motion.position.x - self.position.x,
            0.,
            motion.position.z - self.position.z,
        );
        self.position.x = motion.position.x;
        self.position.z = motion.position.z;
        if motion.blocked {
            self.speed = (movement.dot(movement).sqrt() / dt).min(self.speed);
            if movement.dot(movement) > 1e-10 {
                let heading = movement.x.atan2(movement.z);
                if self.grounded {
                    self.yaw = heading
                        - if self.switch_stance {
                            std::f32::consts::PI
                        } else {
                            0.
                        };
                } else {
                    self.travel_yaw = heading;
                }
            }
        }
        let next_surface = terrain.support(self.position.x, self.position.z, support_ceiling);
        let floor = next_surface.map(|height| height + rails::BOARD_HEIGHT);
        if self.grounded {
            let change = previous_surface
                .zip(next_surface)
                .map(|(previous, next)| next - previous);
            if let Some(change) = change
                && change.abs() <= 0.2
            {
                let standing = Vec3::new(self.position.x, floor.unwrap(), self.position.z);
                if worsens_body_overlap(terrain, previous_position, standing)
                    || (change > 0. && terrain.ceiling_hit(previous_position, standing).is_some())
                {
                    // A required support rise cannot leave the board below its
                    // floor. Reject the horizontal move rather than teleporting.
                    self.position = previous_position;
                    self.speed = 0.;
                } else {
                    self.position.y = standing.y;
                    // Grade changes affect speed while riding the ramp.
                    if movement.dot(movement) > 0.000001 {
                        self.speed = (self.speed - change * 4.0).clamp(0.0, 12.0);
                    }
                }
            } else if change.is_some_and(|change| change > 0.0) {
                self.position.x -= movement.x;
                self.position.z -= movement.z;
                self.speed = 0.0;
            } else {
                self.grounded = false;
                self.vertical_speed = 0.0;
                self.airtime = 0.0;
                self.start_air(false);
            }
        }
        if !self.grounded {
            if input.flip && self.flip_time.is_none() {
                self.flip_time = Some(0.0);
            }
            if let Some(time) = self.flip_time {
                let time = time + dt;
                self.board_roll = (time / 0.45).min(1.0) * std::f32::consts::TAU;
                if time >= 0.45 {
                    self.completed_flips += 1;
                    self.flip_time = None;
                    self.board_roll = 0.0;
                } else {
                    self.flip_time = Some(time);
                }
            }
            self.airtime += dt;
            self.vertical_speed -= 16.0 * dt;
            let from = self.position;
            let to = from + Vec3::new(0., self.vertical_speed * dt, 0.);
            if self.vertical_speed > 0.
                && let Some(hit) = terrain.ceiling_hit(from, to)
            {
                self.position.y =
                    (hit.point.y - crate::collision::BODY_HEAD_HEIGHT - 0.005).clamp(from.y, to.y);
                self.vertical_speed = 0.;
            } else {
                self.position.y = to.y;
            }
            if input.grind
                && self.grind_cooldown <= 0.0
                && self.clean_rotation()
                && let Some(hit) = rails::capture(
                    rails,
                    previous_position,
                    self.position,
                    self.travel_direction(),
                    self.speed,
                )
                && !terrain.blocks_motion(previous_position, self.position)
                && let Some(contact) = rails[hit.rail_index].board_position(hit.along)
                && !terrain.blocks_motion(self.position, contact)
            {
                self.finish_trick();
                self.last_airtime = self.airtime;
                self.speed = hit.speed;
                let rail = rails[hit.rail_index];
                let (axis, _) = rail.axis().unwrap();
                let heading = axis * hit.direction;
                self.travel_yaw = heading.x.atan2(heading.z);
                self.yaw = self.travel_yaw
                    - if self.switch_stance {
                        std::f32::consts::PI
                    } else {
                        0.0
                    };
                self.position = contact;
                self.vertical_speed = 0.0;
                self.last_trick = "50-50 grinding";
                self.grind = Some(Grind {
                    rail_index: hit.rail_index,
                    rail,
                    along: hit.along,
                    direction: hit.direction,
                    balance: 0.0,
                    distance: 0.0,
                    time: 0.0,
                });
                return;
            }
            if let Some(floor) = floor
                && self.position.y <= floor
                && self.vertical_speed <= 0.0
            {
                self.position.y = floor;
                self.vertical_speed = 0.0;
                self.grounded = true;
                if !was_grounded || self.airtime > 0.0 {
                    self.landings += 1;
                    self.last_airtime = self.airtime;
                    self.finish_trick();
                }
            }
        }
    }

    fn start_air(&mut self, ollie: bool) {
        self.airtime = 0.0;
        if self.combo.tricks > 0 {
            self.combo.remaining = BANK_DELAY;
        }
        self.air_was_ollie = ollie;
        self.travel_yaw = self.yaw
            + if self.switch_stance {
                std::f32::consts::PI
            } else {
                0.0
            };
        self.air_spin = 0.0;
        self.completed_flips = 0;
        self.flip_time = None;
        self.board_roll = 0.0;
    }

    fn finish_trick(&mut self) {
        let half_turns = (self.air_spin / std::f32::consts::PI).round();
        if self.clean_rotation() {
            self.switch_stance ^= (half_turns.abs() as u32) % 2 == 1;
            // A wall slide can change air travel without changing trick spin.
            // Reconcile the clean landing with the retained travel direction.
            self.yaw = self.travel_yaw
                - if self.switch_stance {
                    std::f32::consts::PI
                } else {
                    0.
                };
            self.last_trick = match (self.completed_flips > 0, half_turns.abs() >= 1.0) {
                (true, true) => "Flip + spin landed",
                (true, false) => "Kickflip landed",
                (false, true) => "Spin landed",
                (false, false) => {
                    if self.air_was_ollie {
                        "Ollie landed"
                    } else {
                        "Drop landed"
                    }
                }
            };
            if self.air_was_ollie || self.completed_flips > 0 || half_turns.abs() >= 1.0 {
                self.combo
                    .add(100 + self.completed_flips * 300 + half_turns.abs() as u32 * 180);
            }
        } else {
            self.bail("Bail · rotation unfinished");
        }
        self.board_roll = 0.0;
        self.flip_time = None;
    }

    fn clean_rotation(&self) -> bool {
        let half_turns = (self.air_spin / std::f32::consts::PI).round();
        self.flip_time.is_none()
            && (self.air_spin - half_turns * std::f32::consts::PI).abs() <= 30.0f32.to_radians()
    }
    fn exit_grind(&mut self, grind: Grind, pop: bool) {
        if grind.distance >= 0.25 {
            self.combo.add(grind.points());
        }
        self.last_trick = if pop {
            "50-50 pop-off"
        } else {
            "50-50 dismount"
        };
        self.grind_cooldown = 0.25;
        self.grounded = false;
        self.start_air(pop);
        self.vertical_speed = if pop { 5.0 } else { 0.0 };
    }
    fn bail(&mut self, message: &'static str) {
        self.last_trick = message;
        self.speed = 0.0;
        self.vertical_speed = 0.0;
        self.bail_time = 0.8;
        self.board_roll = 0.0;
        self.flip_time = None;
        self.grind = None;
        self.grind_cooldown = 0.25;
        self.combo.clear();
    }
}

/// Center-line body probes prevent new overlap without inventing a full body
/// volume or a depenetration direction. If already overlapping, equal or better
/// nearest vertical clearance permits small escape steps; worse clearance stops.
fn worsens_body_overlap(
    terrain: &(impl crate::collision::Terrain + ?Sized),
    from: Vec3,
    to: Vec3,
) -> bool {
    let clearance = |board| {
        terrain
            .raycast(
                board + Vec3::new(0., 0.02, 0.),
                board + Vec3::new(0., crate::collision::BODY_HEAD_HEIGHT, 0.),
            )
            .map(|hit| hit.fraction)
    };
    match (clearance(from), clearance(to)) {
        (None, Some(_)) => true,
        (Some(previous), Some(next)) => next + 0.0001 < previous,
        _ => false,
    }
}

/// The rendered course uses this same surface definition for the ramp.
pub fn ground_height(x: f32, z: f32) -> f32 {
    if x.abs() > 3.0 {
        return 0.0;
    }
    match z {
        8.0..14.0 => (z - 8.0) / 3.0,
        14.0..=18.0 => 2.0,
        18.0..24.0 => (24.0 - z) / 3.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn obstacle_world(roof: bool) -> crate::collision::TriangleMesh {
        let mut faces = vec![
            [
                Vec3::new(-10., 0., -10.),
                Vec3::new(10., 0., -10.),
                Vec3::new(-10., 0., 10.),
            ],
            [
                Vec3::new(10., 0., -10.),
                Vec3::new(10., 0., 10.),
                Vec3::new(-10., 0., 10.),
            ],
        ];
        if roof {
            faces.extend([
                [
                    Vec3::new(-10., 2.2, -10.),
                    Vec3::new(10., 2.2, -10.),
                    Vec3::new(-10., 2.2, 10.),
                ],
                [
                    Vec3::new(10., 2.2, -10.),
                    Vec3::new(10., 2.2, 10.),
                    Vec3::new(-10., 2.2, 10.),
                ],
            ]);
        } else {
            faces.extend([
                [
                    Vec3::new(0., 0., -10.),
                    Vec3::new(0., 5., -10.),
                    Vec3::new(0., 0., 10.),
                ],
                [
                    Vec3::new(0., 5., -10.),
                    Vec3::new(0., 5., 10.),
                    Vec3::new(0., 0., 10.),
                ],
            ]);
        }
        crate::collision::TriangleMesh::from_triangles(&faces).unwrap()
    }
    #[test]
    fn rising_head_contacts_roof_without_crossing_it_then_lands_once() {
        let mesh = obstacle_world(true);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-10., 0., -10.),
            max: Vec3::new(10., 3., 10.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.12, 0.), 0.).unwrap();
        skater.step_on(
            Input {
                ollie: true,
                ..Default::default()
            },
            FIXED_DT,
            &world,
        );
        let mut peak = skater.position.y;
        for _ in 0..180 {
            skater.step_on(Input::default(), FIXED_DT, &world);
            peak = peak.max(skater.position.y);
            assert!(skater.position.y + crate::collision::BODY_HEAD_HEIGHT <= 2.20001);
        }
        assert!(peak > 0.5 && peak < 0.551);
        assert!(skater.grounded && (skater.position.y - 0.12).abs() < 1e-5);
        assert_eq!(skater.landings, 1);
    }
    #[test]
    fn grounded_wall_contact_retains_tangential_speed_and_heading() {
        let mesh = obstacle_world(false);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-10., 0., -10.),
            max: Vec3::new(10., 5., 10.),
        };
        let mut skater =
            Skater::at_spawn(Vec3::new(-1., 0.12, -2.), std::f32::consts::FRAC_PI_4).unwrap();
        skater.speed = 6.;
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && skater.position.x <= -0.1799 && skater.position.z > 1.);
        assert!(skater.speed > 2. && skater.yaw.abs() < 0.1);
    }
    #[test]
    fn grounded_uphill_motion_cannot_raise_head_through_roof() {
        let mesh = crate::collision::TriangleMesh::from_triangles(&[
            [
                Vec3::new(-2., 0., 0.),
                Vec3::new(2., 0., 0.),
                Vec3::new(-2., 3.2, 8.),
            ],
            [
                Vec3::new(2., 0., 0.),
                Vec3::new(2., 3.2, 8.),
                Vec3::new(-2., 3.2, 8.),
            ],
            [
                Vec3::new(-2., 2., 0.),
                Vec3::new(2., 2., 0.),
                Vec3::new(-2., 2., 8.),
            ],
            [
                Vec3::new(2., 2., 0.),
                Vec3::new(2., 2., 8.),
                Vec3::new(-2., 2., 8.),
            ],
        ])
        .unwrap();
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., 0.),
            max: Vec3::new(2., 4., 8.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.12, 0.), 0.).unwrap();
        skater.speed = 4.;
        for _ in 0..120 {
            skater.step_on(
                Input {
                    push: true,
                    ..Default::default()
                },
                FIXED_DT,
                &world,
            );
            assert!(
                skater.grounded
                    && skater.position.y + crate::collision::BODY_HEAD_HEIGHT <= 2.00001
            );
            assert!((skater.position.y - (0.4 * skater.position.z + 0.12)).abs() < 1e-4);
        }
        assert!(skater.position.z > 0.4 && skater.position.z < 0.58);
    }
    #[test]
    fn grounded_entry_into_low_overhang_stops_before_body_overlap() {
        let mut faces = vec![
            [
                Vec3::new(-2., 0., -3.),
                Vec3::new(2., 0., -3.),
                Vec3::new(-2., 0., 8.),
            ],
            [
                Vec3::new(2., 0., -3.),
                Vec3::new(2., 0., 8.),
                Vec3::new(-2., 0., 8.),
            ],
        ];
        faces.extend([
            [
                Vec3::new(-2., 1.6, 0.),
                Vec3::new(2., 1.6, 0.),
                Vec3::new(-2., 1.6, 8.),
            ],
            [
                Vec3::new(2., 1.6, 0.),
                Vec3::new(2., 1.6, 8.),
                Vec3::new(-2., 1.6, 8.),
            ],
        ]);
        let mesh = crate::collision::TriangleMesh::from_triangles(&faces).unwrap();
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., -3.),
            max: Vec3::new(2., 2., 8.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.12, -1.), 0.).unwrap();
        skater.speed = 6.;
        for _ in 0..120 {
            skater.step_on(
                Input {
                    push: true,
                    ..Default::default()
                },
                FIXED_DT,
                &world,
            );
        }
        assert!(skater.grounded && skater.position.z < 0. && skater.speed == 0.);
    }
    fn low_overhang(slope: f32) -> crate::collision::TriangleMesh {
        let a = Vec3::new(-2., 0., -3.);
        let b = Vec3::new(2., 0., -3.);
        let c = Vec3::new(-2., 0., 4.);
        let d = Vec3::new(2., 0., 4.);
        let e = Vec3::new(-2., 1.5, 0.);
        let f = Vec3::new(2., 1.5, 0.);
        let g = Vec3::new(-2., 1.5 + slope * 2., 2.);
        let h = Vec3::new(2., 1.5 + slope * 2., 2.);
        crate::collision::TriangleMesh::from_triangles(&[
            [a, b, c],
            [b, d, c],
            [e, f, g],
            [f, h, g],
        ])
        .unwrap()
    }
    #[test]
    fn airborne_horizontal_entry_cannot_land_inside_a_low_overhang() {
        let mesh = low_overhang(0.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., -3.),
            max: Vec3::new(2., 2., 4.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.5, -0.2), 0.).unwrap();
        skater.grounded = false;
        skater.vertical_speed = -1.;
        skater.speed = 4.;
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
            assert!(skater.position.z < 0., "Side entry would overlap the roof");
        }
        assert!(skater.grounded && (skater.position.y - 0.12).abs() < 1e-6);
        assert_eq!(skater.landings, 1);
        assert_eq!(skater.combo.value(), 0);
    }
    #[test]
    fn existing_body_overlap_can_escape_in_small_steps_without_depenetration() {
        let mesh = low_overhang(0.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., -3.),
            max: Vec3::new(2., 2., 4.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.12, 0.5), std::f32::consts::PI).unwrap();
        skater.speed = 3.;
        for _ in 0..60 {
            let before = skater.position;
            skater.step_on(Input::default(), FIXED_DT, &world);
            assert!(skater.position.z < before.z);
            assert!((skater.position.y - 0.12).abs() < 1e-6 && skater.grounded);
            assert!((skater.position - before).dot(skater.position - before) < 0.001);
        }
        assert!(skater.position.z < 0.);
        assert_eq!(skater.landings, 0);
    }
    #[test]
    fn existing_overlap_allows_better_clearance_but_rejects_deeper_entry() {
        let mesh = low_overhang(-0.2);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., -3.),
            max: Vec3::new(2., 2., 4.),
        };
        let start = Vec3::new(0., 0.12, 0.5);
        let mut skater = Skater::at_spawn(start, 0.).unwrap();
        skater.speed = 3.;
        skater.step_on(Input::default(), FIXED_DT, &world);
        assert_eq!(skater.position, start);
        assert_eq!(skater.speed, 0.);
        skater.yaw = std::f32::consts::PI;
        skater.speed = 3.;
        skater.step_on(Input::default(), FIXED_DT, &world);
        assert!(skater.position.z < start.z && skater.position.y == start.y);
    }
    #[test]
    fn airborne_wall_slide_keeps_tangent_when_clean_landing_reconciles_stance() {
        let mesh = obstacle_world(false);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-10., 0., -10.),
            max: Vec3::new(10., 5., 10.),
        };
        let mut skater =
            Skater::at_spawn(Vec3::new(-0.6, 0.12, -2.), std::f32::consts::FRAC_PI_4).unwrap();
        skater.speed = 6.;
        skater.step_on(
            Input {
                ollie: true,
                ..Default::default()
            },
            FIXED_DT,
            &world,
        );
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && skater.position.x <= -0.1799 && skater.position.z > 1.);
        assert!(skater.speed > 2. && skater.yaw.abs() < 0.1);
        assert_eq!(skater.landings, 1);
        assert_eq!(skater.last_trick, "Ollie landed");
    }
    fn source_floor(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> crate::collision::TriangleMesh {
        crate::collision::TriangleMesh::from_triangles(&[
            [
                Vec3::new(x0, y, z0),
                Vec3::new(x1, y, z0),
                Vec3::new(x0, y, z1),
            ],
            [
                Vec3::new(x1, y, z0),
                Vec3::new(x1, y, z1),
                Vec3::new(x0, y, z1),
            ],
        ])
        .unwrap()
    }
    #[test]
    fn source_gap_causes_fall_without_floor_or_chain_banking() {
        let mesh = source_floor(-1., 1., 0., 1., 2.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-10., -10., -10.),
            max: Vec3::new(10., 10., 10.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 2.12, 0.9), 0.).unwrap();
        skater.speed = 4.;
        skater.combo.add(400);
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(!skater.grounded && skater.position.y < 0.0 && skater.vertical_speed < 0.0);
        assert_eq!(skater.score, 0);
        assert_eq!(skater.combo.value(), 400);
    }
    #[test]
    fn missing_support_clears_stale_grounding_before_banking_or_bail_recovery() {
        let mesh = source_floor(-1., 1., 0., 1., 0.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-10., -10., -10.),
            max: Vec3::new(10., 10., 10.),
        };
        for bailing in [false, true] {
            let mut skater = Skater::at_spawn(Vec3::new(4., 3., 4.), 0.).unwrap();
            skater.combo.add(400);
            skater.combo.remaining = FIXED_DT * 0.5;
            if bailing {
                skater.bail_time = 0.5;
            }
            skater.step_on(Input::default(), FIXED_DT, &world);
            assert!(!skater.grounded && skater.position.y < 3.0);
            assert_eq!(skater.score, 0);
        }
    }
    #[test]
    fn source_bounds_allow_riding_beyond_the_original_course_limits() {
        let mesh = source_floor(-2., 2., 50., 100., 0.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(-2., 0., 50.),
            max: Vec3::new(2., 1., 100.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(0., 0.12, 60.), 0.).unwrap();
        skater.speed = 8.;
        for _ in 0..120 {
            skater.step_on(Input::default(), FIXED_DT, &world);
        }
        assert!(skater.grounded && skater.position.z > 67.0 && skater.speed > 7.0);
    }
    #[test]
    fn source_world_has_no_invisible_practice_rails() {
        let mesh = source_floor(6., 8., -8., 10., 0.);
        let world = crate::collision::SourceWorld {
            mesh: &mesh,
            min: Vec3::new(6., 0., -8.),
            max: Vec3::new(8., 2., 10.),
        };
        let mut skater = Skater::at_spawn(Vec3::new(7., 0.12, -4.5), 0.).unwrap();
        skater.speed = 6.;
        skater.step_on(
            Input {
                ollie: true,
                grind: true,
                ..Input::default()
            },
            FIXED_DT,
            &world,
        );
        for _ in 0..120 {
            skater.step_on(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
                &world,
            );
            assert!(skater.grind.is_none());
        }
        assert!(skater.grounded);
    }
    #[test]
    fn recovery_clears_transient_states_preserves_banked_points_and_rejects_bad_checkpoint() {
        let mut skater = catch_practice_rail(true);
        skater.score = 1234;
        skater.last_combo = 400;
        skater.landings = 5;
        let previous = skater.position;
        assert!(skater.recover_at(Vec3::new(f32::NAN, 0., 0.), 0.).is_err());
        assert_eq!(skater.position, previous);
        assert!(skater.grind.is_some());
        let spawn = Vec3::new(50., 10.12, 60.);
        skater
            .recover_at(spawn, std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert_eq!(skater.position, spawn);
        assert_eq!(
            (skater.score, skater.last_combo, skater.landings),
            (1234, 400, 5)
        );
        assert!(skater.grounded && skater.grind.is_none() && skater.combo.value() == 0);
        assert!(
            skater.speed == 0.
                && skater.vertical_speed == 0.
                && skater.board_roll == 0.
                && skater.bail_time == 0.
        );
        assert!(skater.travel_direction().x > 0.99);
    }
    #[test]
    fn riding_off_a_ledge_does_not_award_an_ollie() {
        let mut skater = Skater {
            position: Vec3::new(2.99, 2.12, 16.0),
            yaw: std::f32::consts::FRAC_PI_2,
            speed: 5.0,
            ..Skater::default()
        };
        for _ in 0..180 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded);
        assert_eq!(skater.score, 0);
        assert_eq!(skater.last_trick, "Drop landed");
    }
    #[test]
    fn flip_scores_once_after_a_clean_landing() {
        let mut skater = Skater::default();
        skater.step(
            Input {
                ollie: true,
                flip: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..120 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.last_trick, "Kickflip landed");
        assert_eq!(skater.score, 0);
        assert_eq!(skater.combo.value(), 400);
        for _ in 0..240 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, 400);
    }
    fn catch_practice_rail(flip: bool) -> Skater {
        let mut skater = Skater {
            position: Vec3::new(7.0, 0.12, -4.5),
            speed: 6.0,
            ..Skater::default()
        };
        skater.step(
            Input {
                ollie: true,
                flip,
                grind: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..100 {
            skater.step(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
            if skater.grind.is_some() {
                return skater;
            }
        }
        panic!("Ollie should catch the practice rail on descent: {skater:?}");
    }
    #[test]
    fn flip_grind_pop_and_landing_link_once_then_bank() {
        let mut skater = catch_practice_rail(true);
        assert_eq!(skater.combo.tricks, 1);
        assert_eq!(skater.combo.base_points, 400);
        assert!((skater.position.y - 0.77).abs() < 0.001);
        let start = skater.position.z;
        for _ in 0..60 {
            skater.step(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.grind.is_some() && skater.position.z > start + 2.0);
        let grind_points = skater.grind.as_ref().unwrap().points();
        // Pop and hold-button release in the same tick must still pop.
        skater.step(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        assert!(skater.grind.is_none() && skater.vertical_speed > 4.0);
        assert_eq!(skater.combo.tricks, 2);
        assert_eq!(skater.combo.base_points, 400 + grind_points);
        for _ in 0..120 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded);
        assert_eq!(skater.combo.tricks, 3);
        assert_eq!(skater.combo.base_points, 500 + grind_points);
        let expected = (500 + grind_points) * 3;
        for _ in 0..240 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, expected);
        assert_eq!(skater.last_combo, expected);
        assert_eq!(skater.combo.value(), 0);
        for _ in 0..240 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, expected);
    }
    #[test]
    fn rail_end_and_release_dismount_without_duplicate_ollie_points() {
        for release in [true, false] {
            let mut skater = catch_practice_rail(false);
            if release {
                for _ in 0..30 {
                    skater.step(
                        Input {
                            grind: true,
                            ..Input::default()
                        },
                        FIXED_DT,
                    );
                }
                skater.step(Input::default(), FIXED_DT);
            } else {
                for _ in 0..300 {
                    skater.step(
                        Input {
                            grind: true,
                            ..Input::default()
                        },
                        FIXED_DT,
                    );
                    if skater.grind.is_none() {
                        break;
                    }
                }
            }
            assert!(skater.grind.is_none() && !skater.grounded);
            assert!(skater.grind_cooldown > 0.0);
            assert_eq!(skater.combo.tricks, 2);
            let base = skater.combo.base_points;
            // Held grind after leaving the endpoint cannot recapture that rail.
            for _ in 0..60 {
                skater.step(
                    Input {
                        grind: true,
                        ..Input::default()
                    },
                    FIXED_DT,
                );
            }
            assert!(skater.grounded && skater.grind.is_none());
            assert_eq!(skater.combo.base_points, base);
            assert_eq!(skater.combo.tricks, 2);
        }
    }
    #[test]
    fn switch_capture_retains_signed_travel_and_camera_heading() {
        let mut skater = Skater {
            position: Vec3::new(7., 0.9, 5.),
            yaw: 0.0,
            speed: 6.0,
            vertical_speed: -2.0,
            grounded: false,
            switch_stance: true,
            travel_yaw: std::f32::consts::PI,
            ..Skater::default()
        };
        for _ in 0..15 {
            skater.step(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.grind.is_some());
        assert!(skater.travel_direction().z < -0.99);
        let z = skater.position.z;
        for _ in 0..15 {
            skater.step(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.position.z < z - 0.5 && skater.switch_stance);
    }
    #[test]
    fn balance_bail_discards_pending_chain_preserves_bank_and_falls() {
        let mut skater = catch_practice_rail(false);
        skater.score = 1234;
        skater.grind.as_mut().unwrap().balance = 0.999;
        skater.step(
            Input {
                grind: true,
                steer: 1.0,
                ..Input::default()
            },
            FIXED_DT,
        );
        assert!(skater.grind.is_none() && skater.bail_time > 0.0);
        assert_eq!(skater.combo.value(), 0);
        assert_eq!(skater.score, 1234);
        for _ in 0..120 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded && skater.bail_time <= 0.0);
        assert!((skater.position.y - 0.12).abs() < 0.001);
        assert_eq!(skater.score, 1234);
    }
    #[test]
    fn unfinished_flip_and_unheld_grind_do_not_capture() {
        for hold in [false, true] {
            let mut skater = Skater {
                position: Vec3::new(7., 0.82, 2.),
                speed: 6.0,
                vertical_speed: -2.0,
                grounded: false,
                flip_time: if hold { Some(0.0) } else { None },
                ..Skater::default()
            };
            for _ in 0..5 {
                skater.step(
                    Input {
                        grind: hold,
                        ..Input::default()
                    },
                    FIXED_DT,
                );
            }
            assert!(skater.grind.is_none());
        }
    }
    #[test]
    fn new_air_segment_restarts_the_grounded_banking_window() {
        let mut skater = Skater::default();
        skater.combo.add(400);
        for _ in 0..120 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, 0);
        skater.step(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..180 {
            if skater.grounded {
                break;
            }
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded);
        assert_eq!(skater.combo.value(), 1000);
        for _ in 0..179 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, 0);
        skater.step(Input::default(), FIXED_DT);
        assert_eq!(skater.score, 1000);
    }
    #[test]
    fn stalled_capture_cannot_award_a_stationary_grind() {
        let mut skater = catch_practice_rail(false);
        skater.speed = 0.0;
        let before = skater.combo.base_points;
        for _ in 0..60 {
            skater.step(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.grind.is_none() && skater.grounded);
        assert_eq!(skater.combo.base_points, before);
        assert_eq!(skater.combo.tricks, 1);
    }
    #[test]
    fn rail_following_stops_at_an_imported_wall() {
        let mut skater = catch_practice_rail(false);
        let z = skater.position.z + 0.5;
        let mesh = crate::collision::TriangleMesh::from_triangles(&[
            [
                Vec3::new(6., 0., z),
                Vec3::new(8., 0., z),
                Vec3::new(6., 3., z),
            ],
            [
                Vec3::new(8., 0., z),
                Vec3::new(8., 3., z),
                Vec3::new(6., 3., z),
            ],
        ])
        .unwrap();
        for _ in 0..15 {
            skater.step_on(
                Input {
                    grind: true,
                    ..Input::default()
                },
                FIXED_DT,
                &crate::collision::World { mesh: Some(&mesh) },
            );
        }
        assert!(skater.position.z < z && skater.bail_time > 0.0 && skater.grind.is_none());
        assert_eq!(skater.combo.value(), 0);
    }
    #[test]
    fn unfinished_flip_bails_without_awarding_points() {
        let mut skater = Skater::default();
        skater.step(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..65 {
            skater.step(Input::default(), FIXED_DT);
        }
        skater.step(
            Input {
                flip: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..25 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.bail_time > 0.0);
        assert_eq!(skater.score, 0);
    }
    #[test]
    fn half_turn_retains_travel_direction_and_changes_stance() {
        let mut skater = Skater {
            speed: 5.0,
            ..Skater::default()
        };
        skater.step(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        for _ in 0..60 {
            skater.step(
                Input {
                    spin: 1.0,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        for _ in 0..50 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded && skater.switch_stance);
        assert!(skater.position.z > -8.0);
        assert_eq!(skater.last_trick, "Spin landed");
    }
    #[test]
    fn push_brake_and_steer_change_motion() {
        let mut skater = Skater::default();
        for _ in 0..120 {
            skater.step(
                Input {
                    push: true,
                    steer: 0.4,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.speed > 3.0 && skater.position.z > -12.0 && skater.position.x > 0.0);
        for _ in 0..120 {
            skater.step(
                Input {
                    brake: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert_eq!(skater.speed, 0.0);
    }
    #[test]
    fn ollie_returns_to_ground_and_counts_one_landing() {
        let mut skater = Skater::default();
        skater.step(
            Input {
                ollie: true,
                ..Input::default()
            },
            FIXED_DT,
        );
        assert!(!skater.grounded && skater.position.y > 0.12);
        for _ in 0..180 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert!(skater.grounded);
        assert_eq!(skater.landings, 1);
        assert!((skater.position.y - 0.12).abs() < 0.001);
    }
    #[test]
    fn follows_ramp_and_stays_within_course() {
        let mut skater = Skater {
            position: Vec3::new(0.0, 0.12, 7.5),
            speed: 8.0,
            ..Skater::default()
        };
        for _ in 0..200 {
            skater.step(
                Input {
                    push: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.position.y > 1.0 && skater.grounded);
        for _ in 0..1800 {
            skater.step(
                Input {
                    push: true,
                    ..Input::default()
                },
                FIXED_DT,
            );
        }
        assert!(skater.position.z < COURSE_Z && skater.position.y >= 0.12);
    }
}
