//! Original horizontal grind rails for the prototype course.
use crate::skating::Vec3;

pub const BOARD_HEIGHT: f32 = 0.12;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rail {
    /// Top of the riding edge, in prototype metres.
    pub start: Vec3,
    pub end: Vec3,
}

pub const COURSE_RAILS: [Rail; 2] = [
    Rail {
        start: Vec3::new(7.0, 0.65, -2.0),
        end: Vec3::new(7.0, 0.65, 8.0),
    },
    Rail {
        start: Vec3::new(11.0, 0.9, 12.0),
        end: Vec3::new(11.0, 0.9, 24.0),
    },
];

impl Rail {
    /// Only bounded, horizontal, nondegenerate prototype rails are supported.
    pub fn axis(self) -> Option<(Vec3, f32)> {
        if ![self.start, self.end].iter().all(|p| {
            [p.x, p.y, p.z]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1000.0)
        }) || (self.end.y - self.start.y).abs() > 0.0001
        {
            return None;
        }
        let delta = Vec3::new(self.end.x - self.start.x, 0.0, self.end.z - self.start.z);
        let length = delta.dot(delta).sqrt();
        (0.1..=100.0)
            .contains(&length)
            .then(|| (delta * (1.0 / length), length))
    }
    pub fn board_position(self, along: f32) -> Option<Vec3> {
        let (axis, length) = self.axis()?;
        if !along.is_finite() {
            return None;
        }
        Some(self.start + axis * along.clamp(0.0, length) + Vec3::new(0.0, BOARD_HEIGHT, 0.0))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Capture {
    pub rail_index: usize,
    pub along: f32,
    pub direction: f32,
    pub speed: f32,
    pub fraction: f32,
}

/// Sweep the descending board through the rail's height. Merely passing beside
/// or underneath a rail cannot capture it. Direction is the travel heading,
/// which may differ from the board heading in switch stance.
pub fn capture(rails: &[Rail], from: Vec3, to: Vec3, travel: Vec3, speed: f32) -> Option<Capture> {
    if ![from, to, travel]
        .iter()
        .all(|p| [p.x, p.y, p.z].iter().all(|v| v.is_finite()))
        || !speed.is_finite()
        || speed < 2.0
        || to.y >= from.y - 0.000001
    {
        return None;
    }
    let travel = Vec3::new(travel.x, 0.0, travel.z).normalized();
    let mut best: Option<Capture> = None;
    for (rail_index, &rail) in rails.iter().take(256).enumerate() {
        let Some((axis, length)) = rail.axis() else {
            continue;
        };
        let height = rail.start.y + BOARD_HEIGHT;
        if from.y < height || to.y > height {
            continue;
        }
        let fraction = ((height - from.y) / (to.y - from.y)).clamp(0.0, 1.0);
        let point = from + (to - from) * fraction;
        let along = (point - rail.start).dot(axis);
        let alignment = travel.dot(axis);
        if !(0.0..=length).contains(&along) || alignment.abs() < 25.0f32.to_radians().cos() {
            continue;
        }
        let center = rail.start + axis * along;
        let lateral = Vec3::new(point.x - center.x, 0.0, point.z - center.z);
        if lateral.dot(lateral) > 0.32 * 0.32 {
            continue;
        }
        if best.is_none_or(|hit| fraction < hit.fraction) {
            best = Some(Capture {
                rail_index,
                along,
                direction: alignment.signum(),
                speed: speed * alignment.abs(),
                fraction,
            });
        }
    }
    best
}

#[derive(Debug, Clone)]
pub struct Grind {
    pub rail_index: usize,
    pub balance: f32,
    pub distance: f32,
    pub time: f32,
    pub(crate) rail: Rail,
    pub(crate) along: f32,
    pub(crate) direction: f32,
}
impl Grind {
    pub fn points(&self) -> u32 {
        200 + (self.distance * 40.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAIL: Rail = Rail {
        start: Vec3::new(0.0, 0.6, 0.0),
        end: Vec3::new(0.0, 0.6, 10.0),
    };
    #[test]
    fn swept_capture_projects_contact_and_preserves_reverse_travel() {
        let hit = capture(
            &[RAIL],
            Vec3::new(0.1, 0.9, 3.0),
            Vec3::new(0.1, 0.5, 2.6),
            Vec3::new(0.0, 0.0, -1.0),
            6.0,
        )
        .unwrap();
        assert!((hit.fraction - 0.45).abs() < 0.0001);
        assert!((hit.along - 2.82).abs() < 0.0001);
        assert_eq!(hit.direction, -1.0);
        assert_eq!(hit.speed, 6.0);
    }
    #[test]
    fn no_capture_from_below_sideways_outside_or_without_descent() {
        let travel = Vec3::new(0.0, 0.0, 1.0);
        for (from, to, direction, speed) in [
            (Vec3::new(0., 0.5, 2.), Vec3::new(0., 0.3, 2.1), travel, 6.),
            (Vec3::new(0., 0.5, 2.), Vec3::new(0., 0.9, 2.1), travel, 6.),
            (Vec3::new(0., 0.9, 2.), Vec3::new(0., 0.9, 2.1), travel, 6.),
            (
                Vec3::new(0.5, 0.9, 2.),
                Vec3::new(0.5, 0.5, 2.1),
                travel,
                6.,
            ),
            (
                Vec3::new(0., 0.9, -1.),
                Vec3::new(0., 0.5, -0.9),
                travel,
                6.,
            ),
            (
                Vec3::new(0., 0.9, 2.),
                Vec3::new(0., 0.5, 2.1),
                Vec3::new(1., 0., 0.),
                6.,
            ),
            (Vec3::new(0., 0.9, 2.), Vec3::new(0., 0.5, 2.1), travel, 1.),
            (
                Vec3::new(f32::NAN, 0.9, 2.),
                Vec3::new(0., 0.5, 2.1),
                travel,
                6.,
            ),
        ] {
            assert!(capture(&[RAIL], from, to, direction, speed).is_none());
        }
        for bad in [
            Rail {
                end: RAIL.start,
                ..RAIL
            },
            Rail {
                end: Vec3::new(0., 1., 10.),
                ..RAIL
            },
            Rail {
                end: Vec3::new(0., 0.6, f32::INFINITY),
                ..RAIL
            },
        ] {
            assert!(bad.axis().is_none());
        }
    }
}
