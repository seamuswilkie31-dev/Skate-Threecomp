//! Native 3D pass with its own depth attachment, composited into the launcher.
use eframe::{
    egui,
    egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor},
    wgpu,
};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera {
    pub eye: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub forward: [f32; 4],
    pub projection: [f32; 4],
}

pub struct Frame {
    pub vertices: Vec<Vertex>,
    pub camera: Camera,
    pub size: egui::Vec2,
    pub target: wgpu::TextureFormat,
}

struct Targets {
    size: [u32; 2],
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    blit_group: wgpu::BindGroup,
}
struct Renderer {
    world: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    targets: Targets,
    vertices: wgpu::Buffer,
    vertex_capacity: usize,
}

impl CallbackTrait for Frame {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<Renderer>().is_none() {
            resources.insert(Renderer::new(device, self.target));
        }
        let renderer = resources.get_mut::<Renderer>().unwrap();
        let max = device.limits().max_texture_dimension_2d.min(4096);
        let size = [
            (self.size.x * screen.pixels_per_point).round() as u32,
            (self.size.y * screen.pixels_per_point).round() as u32,
        ]
        .map(|v| v.clamp(1, max));
        renderer.draw(device, queue, encoder, size, &self.vertices, &self.camera);
        Vec::new()
    }
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<Renderer>() {
            pass.set_pipeline(&renderer.blit);
            pass.set_bind_group(0, &renderer.targets.blit_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

impl Renderer {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Threecomp world shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Threecomp compositing shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3];
        let world = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Threecomp depth-tested world"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Threecomp viewport composite"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Threecomp camera"),
            size: std::mem::size_of::<Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Threecomp camera binding"),
            layout: &world.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let targets = Targets::new(device, &blit, [1, 1]);
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Threecomp scene vertices"),
            size: 24,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            world,
            blit,
            uniform,
            camera_group,
            targets,
            vertices,
            vertex_capacity: 1,
        }
    }
    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        size: [u32; 2],
        vertices: &[Vertex],
        camera: &Camera,
    ) {
        if self.targets.size != size {
            self.targets = Targets::new(device, &self.blit, size);
        }
        if vertices.len() > self.vertex_capacity {
            self.vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Threecomp scene vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            self.vertex_capacity = vertices.len();
        } else if !vertices.is_empty() {
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(camera));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Threecomp 3D pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 126.0 / 255.0,
                        g: 160.0 / 255.0,
                        b: 177.0 / 255.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.targets.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.world);
        pass.set_bind_group(0, &self.camera_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }
}

impl Targets {
    fn new(device: &wgpu::Device, blit: &wgpu::RenderPipeline, size: [u32; 2]) -> Self {
        let create = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = create(
            "Threecomp viewport color",
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = create(
            "Threecomp viewport depth",
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Threecomp viewport sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blit_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Threecomp viewport binding"),
            layout: &blit.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            size,
            _color: color,
            color_view,
            _depth: depth,
            depth_view,
            blit_group,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires a native GPU and THREECOMP_TEST_ISO pointing to a local retail disc image"]
    fn retail_geometry_sample_draws_on_native_gpu() {
        pollster::block_on(async {
            let path = std::env::var_os("THREECOMP_TEST_ISO")
                .expect("Set THREECOMP_TEST_ISO to your local disc image");
            let sample =
                threecomp_engine_core::geometry::load_retail_sample(std::path::Path::new(&path))
                    .unwrap();
            let center = (sample.min + sample.max) * 0.5;
            let extent = sample.max - sample.min;
            let scale = 2.0 / extent.x.max(extent.y).max(extent.z).max(0.001);
            let mut vertices = Vec::new();
            for mesh in &sample.meshes {
                for &index in &mesh.indices {
                    let p = (mesh.positions[index as usize] - center) * scale;
                    vertices.push(Vertex {
                        position: [p.x, p.y, p.z],
                        color: [0.7, 0.8, 0.9],
                    });
                }
            }
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&Default::default())
                .await
                .expect("Native GPU adapter required");
            let (device, queue) = adapter
                .request_device(&Default::default(), None)
                .await
                .unwrap();
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            let camera = Camera {
                eye: [0., 0., -3., 0.],
                right: [1., 0., 0., 0.],
                up: [0., 1., 0., 0.],
                forward: [0., 0., 1., 0.],
                projection: [1.8, 1.8, 0.15, 100.],
            };
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.draw(&device, &queue, &mut encoder, [64, 64], &vertices, &camera);
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Retail geometry test pixels"),
                size: 256 * 64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &renderer.targets._color,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(64),
                    },
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (sender, receiver) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            device.poll(wgpu::Maintain::Wait);
            receiver
                .recv_timeout(std::time::Duration::from_secs(15))
                .unwrap()
                .unwrap();
            let data = readback.slice(..).get_mapped_range();
            let drawn = data
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 170 && pixel[1] > 190)
                .count();
            assert!(drawn > 10, "Expected real geometry pixels, got {drawn}");
            drop(data);
            readback.unmap();
            assert!(device.pop_error_scope().await.is_none());
            eprintln!(
                "GPU rendered {} meshes / {} triangles into {drawn} sample pixels",
                sample.meshes.len(),
                vertices.len() / 3
            );
        });
    }
    #[test]
    #[ignore = "Requires a native GPU adapter; run explicitly on the development machine"]
    fn depth_occlusion_and_compositing_on_native_gpu() {
        pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("Native GPU adapter required");
            eprintln!("GPU validation adapter: {}", adapter.get_info().name);
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .unwrap();
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            let camera = Camera {
                eye: [0.; 4],
                right: [1., 0., 0., 0.],
                up: [0., 1., 0., 0.],
                forward: [0., 0., 1., 0.],
                projection: [1., 1., 0.15, 100.],
            };
            // The farther green triangle is deliberately submitted AFTER red.
            let mut vertices = Vec::new();
            for (z, color) in [(2., [1., 0., 0.]), (4., [0., 1., 0.])] {
                for (x, y) in [(-0.5, -0.5), (0.5, -0.5), (0., 0.5)] {
                    vertices.push(Vertex {
                        position: [x * z, y * z, z],
                        color,
                    });
                }
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.draw(&device, &queue, &mut encoder, [32, 32], &vertices, &camera);
            let output = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("GPU test composite"),
                size: wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = output.create_view(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("GPU test composite"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&renderer.blit);
                pass.set_bind_group(0, &renderer.targets.blit_group, &[]);
                pass.draw(0..3, 0..1);
            }
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU test pixels"),
                size: 256 * 32,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &output,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(32),
                    },
                },
                wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (sender, receiver) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            device.poll(wgpu::Maintain::Wait);
            receiver
                .recv_timeout(std::time::Duration::from_secs(15))
                .unwrap()
                .unwrap();
            let data = readback.slice(..).get_mapped_range();
            assert_eq!(
                &data[16 * 256 + 16 * 4..16 * 256 + 16 * 4 + 4],
                &[255, 0, 0, 255],
                "Near triangle must occlude far triangle in the composited image"
            );
            drop(data);
            readback.unmap();
            assert!(
                device.pop_error_scope().await.is_none(),
                "GPU validation error"
            );
        });
    }
}
