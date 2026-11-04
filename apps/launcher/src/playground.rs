//! Native prototype view, rendered as projected triangles by egui's wgpu backend.

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use threecomp_engine_core::skating::{FIXED_DT, Input, Skater, Vec3, ground_height};

pub struct Playground {
    skater: Skater,
    accumulator: f32,
    pending_ollie: bool,
    pending_flip: bool,
    controllers: crate::controllers::Controllers,
    camera: Vec3,
    target: Vec3,
}

impl Default for Playground {
    fn default() -> Self {
        let skater = Skater::default();
        let camera = skater.position + Vec3::new(0.0, 3.5, -7.0);
        let target = skater.position + Vec3::new(0.0, 0.8, 2.0);
        Self {
            skater,
            accumulator: 0.0,
            pending_ollie: false,
            pending_flip: false,
            controllers: crate::controllers::Controllers::default(),
            camera,
            target,
        }
    }
}

impl Playground {
    fn reset(&mut self) {
        self.skater = Skater::default();
        self.accumulator = 0.0;
        self.pending_ollie = false;
        self.pending_flip = false;
        self.camera = self.skater.position + v(0.0, 3.5, -7.0);
        self.target = self.skater.position + v(0.0, 0.8, 2.0);
    }
    /// Return false when the player asks to return to the launcher.
    pub fn update(&mut self, ctx: &egui::Context) -> bool {
        let mut playing = true;
        egui::TopBottomPanel::top("prototype_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("THREECOMP / SKATING PROTOTYPE");
                ui.separator();
                if ui.button("Launcher (Esc)").clicked() {
                    playing = false;
                }
                if ui.button("Reset (R)").clicked() {
                    self.reset();
                }
            });
            ui.label("Original test course · keyboard + controller · early movement model");
            ui.small(&self.controllers.status);
        });
        egui::TopBottomPanel::bottom("prototype_controls").show(ctx, |ui| {
            ui.label("W/S Push/brake · A/D Steer · Space Ollie · F Flip · Q/E Air spin · R Reset");
            ui.small("Pad: stick Steer · A Push · B Brake · X Ollie · Y Flip · LB/RB Spin · Start Reset · Back Launcher");
            ui.horizontal(|ui| {
                ui.strong(format!("{:.1} km/h", self.skater.speed * 3.6));
                ui.separator();
                ui.label(if self.skater.bail_time>0.0 { "BAIL" } else if self.skater.grounded {
                    "RIDING"
                } else {
                    "AIRBORNE"
                });
                ui.label(format!(
                    "Landings: {}   Last air: {:.2}s",
                    self.skater.landings, self.skater.last_airtime
                ));
            });
            ui.label(format!("Score: {} · {} · {}",self.skater.score,self.skater.last_trick,if self.skater.switch_stance { "Switch" } else { "Regular" }));
        });
        let (dt, focused, reset, escape, jump, flip, mut input) = ctx.input(|i| {
            let left = i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft);
            let right = i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight);
            (
                i.stable_dt.min(0.1),
                i.viewport().focused.unwrap_or(true),
                i.key_pressed(egui::Key::R),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::F),
                Input {
                    push: i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp),
                    brake: i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown),
                    steer: f32::from(right) - f32::from(left),
                    ollie: false,
                    flip: false,
                    spin: f32::from(i.key_down(egui::Key::E)) - f32::from(i.key_down(egui::Key::Q)),
                },
            )
        });
        let controller = self.controllers.poll(focused);
        input.push |= controller.input.push;
        input.brake |= controller.input.brake;
        input.steer = (input.steer + controller.input.steer).clamp(-1.0, 1.0);
        input.spin = (input.spin + controller.input.spin).clamp(-1.0, 1.0);
        if reset || controller.reset {
            self.reset();
        }
        if escape || controller.menu {
            playing = false;
        }
        if focused && playing {
            self.pending_ollie |= jump || controller.input.ollie;
            self.pending_flip |= flip || controller.input.flip;
            self.accumulator += dt;
            while self.accumulator >= FIXED_DT {
                self.skater.step(
                    Input {
                        ollie: self.pending_ollie,
                        flip: self.pending_flip,
                        ..input
                    },
                    FIXED_DT,
                );
                self.pending_ollie = false;
                self.pending_flip = false;
                self.accumulator -= FIXED_DT;
            }
            let blend = 1.0 - (-6.0 * dt).exp();
            // Keep the follow camera steady while the rider spins in the air.
            let heading = self.skater.travel_direction();
            let desired = self.skater.position - heading * 7.0 + Vec3::new(0.0, 3.5, 0.0);
            self.camera = self.camera + (desired - self.camera) * blend;
            let target = self.skater.position + heading * 2.0 + Vec3::new(0.0, 0.8, 0.0);
            self.target = self.target + (target - self.target) * blend;
        } else {
            self.pending_ollie = false;
            self.pending_flip = false;
            self.accumulator = 0.0;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, Color32::from_rgb(126, 160, 177));
                let mut scene = Scene::new(rect, self.camera, self.target);
                for x in -10..10 {
                    for z in -20..20 {
                        let (x, z) = (x as f32 * 2.0, z as f32 * 2.0);
                        let color = if ((x as i32 / 2) + (z as i32 / 2)) & 1 == 0 {
                            [80, 99, 93]
                        } else {
                            [87, 106, 100]
                        };
                        scene.quad(
                            [
                                v(x, 0.0, z),
                                v(x + 2.0, 0.0, z),
                                v(x + 2.0, 0.0, z + 2.0),
                                v(x, 0.0, z + 2.0),
                            ],
                            color,
                        );
                    }
                }
                for (start, end) in [(8.0, 14.0), (14.0, 18.0), (18.0, 24.0)] {
                    let y0 = ground_height(0.0, start);
                    let y1 = ground_height(0.0, end);
                    scene.quad(
                        [
                            v(-3.0, y0, start),
                            v(3.0, y0, start),
                            v(3.0, y1, end),
                            v(-3.0, y1, end),
                        ],
                        [166, 184, 140],
                    );
                    for side in [-3.0, 3.0] {
                        scene.quad(
                            [
                                v(side, 0.0, start),
                                v(side, y0, start),
                                v(side, y1, end),
                                v(side, 0.0, end),
                            ],
                            [99, 113, 81],
                        );
                    }
                }
                for x in [-20.0, 20.0] {
                    scene.cuboid(v(x, 0.3, 0.0), v(0.2, 0.6, 80.0), 0.0, [214, 126, 92]);
                }
                for z in [-40.0, 40.0] {
                    scene.cuboid(v(0.0, 0.3, z), v(40.0, 0.6, 0.2), 0.0, [214, 126, 92]);
                }
                // Start and finish stripes help judge speed and direction.
                for z in [-8.0, 28.0] {
                    scene.quad(
                        [
                            v(-10.0, 0.015, z),
                            v(10.0, 0.015, z),
                            v(10.0, 0.015, z + 0.25),
                            v(-10.0, 0.015, z + 0.25),
                        ],
                        [218, 231, 185],
                    );
                }
                let p = self.skater.position;
                let yaw = self.skater.yaw;
                let surface = ground_height(p.x, p.z);
                scene.quad(
                    [
                        v(p.x - 0.35, surface + 0.02, p.z - 0.65),
                        v(p.x + 0.35, surface + 0.02, p.z - 0.65),
                        v(p.x + 0.35, surface + 0.02, p.z + 0.65),
                        v(p.x - 0.35, surface + 0.02, p.z + 0.65),
                    ],
                    [51, 64, 62],
                );
                let roll = self.skater.board_roll;
                scene.cuboid_rolled(p, v(0.28, 0.09, 0.88), yaw, roll, [194, 239, 97]);
                for dx in [-0.18, 0.18] {
                    for dz in [-0.29, 0.29] {
                        scene.cuboid_rolled(
                            p + rotate(roll_point(v(dx, -0.065, dz), roll), yaw),
                            v(0.1, 0.1, 0.12),
                            yaw,
                            roll,
                            [33, 39, 41],
                        );
                    }
                }
                for dz in [-0.22, 0.22] {
                    scene.cuboid(
                        p + rotate(v(0.0, 0.35, dz), yaw),
                        v(0.18, 0.55, 0.18),
                        yaw,
                        [44, 58, 77],
                    );
                }
                let lean = input.steer * 0.12;
                scene.cuboid(
                    p + rotate(v(lean, 0.94, 0.0), yaw),
                    v(0.45, 0.65, 0.3),
                    yaw,
                    [225, 123, 89],
                );
                scene.cuboid(
                    p + rotate(v(lean, 1.43, 0.0), yaw),
                    v(0.28, 0.29, 0.28),
                    yaw,
                    [234, 203, 163],
                );
                for dx in [-0.32, 0.32] {
                    scene.cuboid(
                        p + rotate(v(dx + lean, 0.89, 0.0), yaw),
                        v(0.15, 0.5, 0.15),
                        yaw,
                        [234, 203, 163],
                    );
                }
                scene.paint(&painter);
                if !focused {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Paused · click the window to skate",
                        egui::FontId::proportional(22.0),
                        Color32::WHITE,
                    );
                }
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(if focused {
            8
        } else {
            100
        }));
        playing
    }
}

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}
fn rotate(p: Vec3, yaw: f32) -> Vec3 {
    v(
        p.x * yaw.cos() + p.z * yaw.sin(),
        p.y,
        -p.x * yaw.sin() + p.z * yaw.cos(),
    )
}
fn roll_point(p: Vec3, roll: f32) -> Vec3 {
    v(
        p.x * roll.cos() - p.y * roll.sin(),
        p.x * roll.sin() + p.y * roll.cos(),
        p.z,
    )
}

struct Face {
    points: Vec<Vec3>,
    color: Color32,
    depth: f32,
}
struct Scene {
    rect: Rect,
    eye: Vec3,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    faces: Vec<Face>,
}
impl Scene {
    fn new(rect: Rect, eye: Vec3, target: Vec3) -> Self {
        let forward = (target - eye).normalized();
        let right = v(0.0, 1.0, 0.0).cross(forward).normalized();
        Self {
            rect,
            eye,
            forward,
            right,
            up: forward.cross(right),
            faces: Vec::new(),
        }
    }
    fn quad(&mut self, points: [Vec3; 4], color: [u8; 3]) {
        let points: Vec<_> = points
            .into_iter()
            .map(|p| {
                let d = p - self.eye;
                v(d.dot(self.right), d.dot(self.up), d.dot(self.forward))
            })
            .collect();
        // Clip convex faces to the near plane before perspective projection.
        let mut clipped = Vec::new();
        for index in 0..points.len() {
            let a = points[index];
            let b = points[(index + 1) % points.len()];
            if a.z >= 0.15 {
                clipped.push(a);
            }
            if (a.z >= 0.15) != (b.z >= 0.15) {
                clipped.push(a + (b - a) * ((0.15 - a.z) / (b.z - a.z)));
            }
        }
        if clipped.len() < 3 {
            return;
        }
        let depth = clipped.iter().map(|p| p.z).sum::<f32>() / clipped.len() as f32;
        self.faces.push(Face {
            points: clipped,
            color: Color32::from_rgb(color[0], color[1], color[2]),
            depth,
        });
    }
    fn cuboid(&mut self, center: Vec3, size: Vec3, yaw: f32, color: [u8; 3]) {
        self.cuboid_rolled(center, size, yaw, 0.0, color);
    }
    fn cuboid_rolled(&mut self, center: Vec3, size: Vec3, yaw: f32, roll: f32, color: [u8; 3]) {
        let points: Vec<_> = [
            (-1., -1., -1.),
            (1., -1., -1.),
            (1., 1., -1.),
            (-1., 1., -1.),
            (-1., -1., 1.),
            (1., -1., 1.),
            (1., 1., 1.),
            (-1., 1., 1.),
        ]
        .into_iter()
        .map(|(x, y, z)| {
            center
                + rotate(
                    roll_point(v(x * size.x / 2., y * size.y / 2., z * size.z / 2.), roll),
                    yaw,
                )
        })
        .collect();
        for (face, shade) in [
            ([0, 1, 2, 3], 0.7),
            ([4, 5, 6, 7], 0.9),
            ([0, 4, 7, 3], 0.8),
            ([1, 5, 6, 2], 0.85),
            ([3, 2, 6, 7], 1.0),
            ([0, 1, 5, 4], 0.6),
        ] {
            self.quad(
                face.map(|index| points[index]),
                color.map(|c| (c as f32 * shade) as u8),
            );
        }
    }
    fn paint(mut self, painter: &egui::Painter) {
        self.faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        let scale = self.rect.width().min(self.rect.height()) * 0.9;
        let mut mesh = egui::Mesh::default();
        for face in self.faces {
            let start = mesh.vertices.len() as u32;
            for point in &face.points {
                let screen =
                    self.rect.center() + Vec2::new(point.x / point.z, -point.y / point.z) * scale;
                mesh.colored_vertex(Pos2::new(screen.x, screen.y), face.color);
            }
            for index in 1..face.points.len() - 1 {
                mesh.add_triangle(start, start + index as u32, start + index as u32 + 1);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }
}
