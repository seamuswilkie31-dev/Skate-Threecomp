//! Original skating movement for a synthetic course, independent of game assets.

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
    pub last_trick: &'static str,
    /// Displayed board roll during an active flip.
    pub board_roll: f32,
    flip_time: Option<f32>,
    completed_flips: u32,
    air_spin: f32,
    travel_yaw: f32,
    pub bail_time: f32,
    pub switch_stance: bool,
    air_start_yaw: f32,
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
            last_trick: "Ready",
            board_roll: 0.0,
            flip_time: None,
            completed_flips: 0,
            air_spin: 0.0,
            travel_yaw: 0.0,
            bail_time: 0.0,
            switch_stance: false,
            air_start_yaw: 0.0,
            air_was_ollie: false,
        }
    }
}
impl Skater {
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
    pub fn step_on(&mut self, input: Input, dt: f32, terrain: &impl crate::collision::Terrain) {
        if !dt.is_finite() || !(0.0..=1.0 / 30.0).contains(&dt) {
            return;
        }
        let was_grounded = self.grounded;
        if self.bail_time > 0.0 {
            self.bail_time = (self.bail_time - dt).max(0.0);
            self.speed = 0.0;
            return;
        }
        let steer = if input.steer.is_finite() {
            input.steer.clamp(-1.0, 1.0)
        } else {
            0.0
        };
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
        let support_ceiling = self.position.y + 0.08;
        let previous_surface = terrain.height(self.position.x, self.position.z, support_ceiling);
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
        let movement = Vec3::new(heading.sin(), 0.0, heading.cos()) * (self.speed * dt);
        let next = self.position + movement;
        if next.x.abs() > COURSE_X - 0.5
            || next.z.abs() > COURSE_Z - 0.5
            || terrain.blocks_motion(self.position, next)
        {
            self.speed = 0.0;
        } else {
            self.position.x = next.x;
            self.position.z = next.z;
        }
        let floor = terrain.height(self.position.x, self.position.z, support_ceiling) + 0.12;
        if self.grounded {
            let change = floor - 0.12 - previous_surface;
            if change.abs() <= 0.2 {
                self.position.y = floor;
                // Grade changes affect speed while riding the ramp.
                if movement.dot(movement) > 0.000001 {
                    self.speed = (self.speed - change * 4.0).clamp(0.0, 12.0);
                }
            } else if change > 0.0 {
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
            self.position.y += self.vertical_speed * dt;
            if self.position.y <= floor && self.vertical_speed <= 0.0 {
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
        self.air_was_ollie = ollie;
        self.air_start_yaw = self.yaw;
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
        let error = (self.air_spin - half_turns * std::f32::consts::PI).abs();
        let clean = self.flip_time.is_none() && error <= 30.0f32.to_radians();
        if clean {
            self.yaw = self.air_start_yaw + half_turns * std::f32::consts::PI;
            self.switch_stance ^= (half_turns.abs() as u32) % 2 == 1;
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
                self.score = self.score.saturating_add(
                    100 + self.completed_flips * 300 + half_turns.abs() as u32 * 180,
                );
            }
        } else {
            self.last_trick = "Bail · rotation unfinished";
            self.speed = 0.0;
            self.bail_time = 0.8;
        }
        self.board_roll = 0.0;
        self.flip_time = None;
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
        assert_eq!(skater.score, 400);
        for _ in 0..120 {
            skater.step(Input::default(), FIXED_DT);
        }
        assert_eq!(skater.score, 400);
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
