//! Optional native controller input. Keyboard controls remain available.
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use threecomp_engine_core::skating::Input;

#[derive(Default)]
pub struct Frame {
    pub input: Input,
    pub reset: bool,
    pub menu: bool,
    pub camera_look: [f32; 2],
    pub camera_reset: bool,
}

pub struct Controllers {
    context: Option<Gilrs>,
    active: Option<GamepadId>,
    edges: Edges,
    pub status: String,
}

impl Default for Controllers {
    fn default() -> Self {
        match Gilrs::new() {
            Ok(context) => Self {
                context: Some(context),
                active: None,
                edges: Edges::default(),
                status: "No controller connected".into(),
            },
            Err(error) => Self {
                context: None,
                active: None,
                edges: Edges::default(),
                status: format!("Controller unavailable: {error}"),
            },
        }
    }
}

impl Controllers {
    pub fn poll(&mut self, focused: bool) -> Frame {
        let Some(context) = &mut self.context else {
            return Frame::default();
        };
        let old_active = self.active;
        for _ in 0..256 {
            let Some(event) = context.next_event() else {
                break;
            };
            let meaningful = match event.event {
                EventType::ButtonPressed(_, _) => true,
                EventType::AxisChanged(axis, value, _) => {
                    matches!(
                        axis,
                        Axis::LeftStickX | Axis::RightStickX | Axis::RightStickY
                    ) && value.abs() > 0.4
                }
                _ => false,
            };
            if meaningful {
                self.active = Some(event.id);
            }
        }
        if self
            .active
            .is_none_or(|id| !context.gamepad(id).is_connected())
        {
            self.active = context.gamepads().next().map(|(id, _)| id);
        }
        let Some(id) = self.active else {
            self.edges = Edges::default();
            self.status = "No controller connected".into();
            return Frame::default();
        };
        let pad = context.gamepad(id);
        self.status = format!("Controller: {}", pad.name());
        let pressed = u8::from(pad.is_pressed(Button::West))
            | (u8::from(pad.is_pressed(Button::North)) << 1)
            | (u8::from(pad.is_pressed(Button::Start)) << 2)
            | (u8::from(pad.is_pressed(Button::Select)) << 3)
            | (u8::from(pad.is_pressed(Button::RightThumb)) << 4);
        let edges = self
            .edges
            .update(pressed, focused && old_active == self.active);
        if !focused {
            return Frame::default();
        }
        Frame {
            input: Input {
                steer: deadzone(pad.value(Axis::LeftStickX)),
                push: pad.is_pressed(Button::South),
                brake: pad.is_pressed(Button::East),
                ollie: edges & 1 != 0,
                flip: edges & 2 != 0,
                spin: f32::from(pad.is_pressed(Button::RightTrigger))
                    - f32::from(pad.is_pressed(Button::LeftTrigger)),
                grind: pad.is_pressed(Button::LeftTrigger2),
            },
            reset: edges & 4 != 0,
            menu: edges & 8 != 0,
            camera_look: [
                deadzone(pad.value(Axis::RightStickX)),
                deadzone(pad.value(Axis::RightStickY)),
            ],
            camera_reset: edges & 16 != 0,
        }
    }
}

#[derive(Default)]
struct Edges {
    previous: u8,
}
impl Edges {
    fn update(&mut self, pressed: u8, enabled: bool) -> u8 {
        let edges = if enabled { pressed & !self.previous } else { 0 };
        self.previous = pressed;
        edges
    }
}
fn deadzone(value: f32) -> f32 {
    if !value.is_finite() || value.abs() <= 0.18 {
        0.0
    } else {
        value.signum() * ((value.abs() - 0.18) / 0.82).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadzone_is_bounded_and_symmetric() {
        assert_eq!(deadzone(0.1), 0.0);
        assert_eq!(deadzone(f32::NAN), 0.0);
        assert_eq!(deadzone(2.0), 1.0);
        assert_eq!(deadzone(-1.0), -1.0);
        assert_eq!(deadzone(0.6), -deadzone(-0.6));
    }
    #[test]
    fn edges_require_release_after_focus_or_controller_changes() {
        let mut edges = Edges::default();
        assert_eq!(edges.update(1, false), 0);
        assert_eq!(edges.update(1, true), 0);
        assert_eq!(edges.update(0, true), 0);
        assert_eq!(edges.update(1, true), 1);
        assert_eq!(edges.update(1, true), 0);
        assert_eq!(edges.update(3, false), 0);
        assert_eq!(edges.update(3, true), 0);
    }
}
