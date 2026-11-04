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
        }
    }
}
impl Skater {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
    /// Called at a fixed 120 Hz by the playground. Invalid or large caller steps
    /// are ignored so a paused window cannot destabilize collision integration.
    pub fn step(&mut self, input: Input, dt: f32) {
        if !dt.is_finite() || !(0.0..=1.0 / 30.0).contains(&dt) {
            return;
        }
        let was_grounded = self.grounded;
        let steer = if input.steer.is_finite() {
            input.steer.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.yaw += steer * (0.6 + self.speed * 0.09) * if self.grounded { 1.0 } else { 0.25 } * dt;
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
            }
        }
        let previous_surface = ground_height(self.position.x, self.position.z);
        let movement = self.forward() * (self.speed * dt);
        let next = self.position + movement;
        if next.x.abs() > COURSE_X - 0.5 || next.z.abs() > COURSE_Z - 0.5 {
            self.speed = 0.0;
        } else {
            self.position.x = next.x;
            self.position.z = next.z;
        }
        let floor = ground_height(self.position.x, self.position.z) + 0.12;
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
            }
        }
        if !self.grounded {
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
                }
            }
        }
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
