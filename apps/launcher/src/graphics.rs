//! Native 3D pass with its own depth attachment, composited into the launcher.
use eframe::{
    egui,
    egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor},
    wgpu,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub layer: u32,
}
pub struct TextureSet {
    pub id: u64,
    pub layers: Vec<Vec<u8>>,
}
impl TextureSet {
    pub fn from_assets(assets: &[threecomp_engine_core::texture::TextureAsset]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut layers = vec![vec![255; 64 * 64 * 4]];
        for asset in assets
            .iter()
            .take(threecomp_engine_core::scene::MAX_TEXTURES)
        {
            let image = &asset.pixels;
            let mut layer = vec![0; 64 * 64 * 4];
            for y in 0..64 {
                for x in 0..64 {
                    let source = (((y * image.height / 64) * image.width + x * image.width / 64)
                        * 4) as usize;
                    let destination = ((y * 64 + x) * 4) as usize;
                    layer[destination..destination + 4]
                        .copy_from_slice(&image.rgba8[source..source + 4]);
                }
            }
            layers.push(layer);
        }
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            layers,
        }
    }
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
    pub textures: Option<Arc<TextureSet>>,
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
    texture_id: Option<u64>,
    _textures: wgpu::Texture,
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
        renderer.set_textures(device, queue, self.textures.as_deref());
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
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32];
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
        let textures = make_texture_array(device, 1);
        let camera_group = texture_camera_group(device, &world, &uniform, &textures);
        let targets = Targets::new(device, &blit, [1, 1]);
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Threecomp scene vertices"),
            size: std::mem::size_of::<Vertex>() as u64,
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
            texture_id: None,
            _textures: textures,
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
        if self.texture_id.is_none() {
            self.set_textures(device, queue, None);
        }
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
    fn set_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: Option<&TextureSet>,
    ) {
        let id = textures.map_or(0, |set| set.id);
        if self.texture_id == Some(id) {
            return;
        }
        let white = vec![vec![255; 64 * 64 * 4]];
        let layers = textures.map_or(&white, |set| &set.layers);
        self._textures = make_texture_array(device, layers.len() as u32);
        for (layer, data) in layers.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self._textures,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.camera_group =
            texture_camera_group(device, &self.world, &self.uniform, &self._textures);
        self.texture_id = Some(id);
    }
}
fn make_texture_array(device: &wgpu::Device, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Threecomp diffuse array"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
fn texture_camera_group(
    device: &wgpu::Device,
    pipeline: &wgpu::RenderPipeline,
    uniform: &wgpu::Buffer,
    texture: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Threecomp camera and diffuse bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
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
        render_retail_geometry(false);
    }
    #[test]
    #[ignore = "Requires a native GPU and THREECOMP_TEST_ISO pointing to a local retail disc image"]
    fn retail_neighborhood_draws_on_native_gpu() {
        render_retail_geometry(true);
    }
    fn render_retail_geometry(neighborhood: bool) {
        pollster::block_on(async {
            let path = std::env::var_os("THREECOMP_TEST_ISO")
                .expect("Set THREECOMP_TEST_ISO to your local disc image");
            let (sample, assets, layers) = if neighborhood {
                let patch = threecomp_engine_core::scene::load_retail_neighborhood(
                    std::path::Path::new(&path),
                )
                .unwrap();
                assert_eq!(patch.geometry.meshes.len(), 64);
                (patch.geometry, patch.assets, patch.mesh_layers)
            } else {
                let sample = threecomp_engine_core::geometry::load_retail_sample(
                    std::path::Path::new(&path),
                )
                .unwrap();
                let assets = threecomp_engine_core::texture::load_retail_diffuse_textures(
                    std::path::Path::new(&path),
                )
                .unwrap();
                let bindings = threecomp_engine_core::materials::inspect_materials(
                    &threecomp_engine_core::geometry::load_retail_block(
                        std::path::Path::new(&path),
                        15,
                    )
                    .unwrap(),
                )
                .unwrap();
                let layers = sample
                    .meshes
                    .iter()
                    .map(|mesh| {
                        bindings
                            .iter()
                            .find(|b| b.draw_index == mesh.draw_index)
                            .and_then(|b| b.diffuse_key)
                            .and_then(|key| assets.iter().position(|a| a.key == key))
                            .map_or(0, |i| i as u32 + 1)
                    })
                    .collect();
                (sample, assets, layers)
            };
            let center = (sample.min + sample.max) * 0.5;
            let extent = sample.max - sample.min;
            let scale = 2.0 / extent.x.max(extent.y).max(extent.z).max(0.001);
            let mut vertices = Vec::new();
            let mut textured_meshes = 0;
            for (mesh, &layer) in sample.meshes.iter().zip(&layers) {
                if layer != 0 {
                    textured_meshes += 1;
                }
                for &index in &mesh.indices {
                    let p = (mesh.positions[index as usize] - center) * scale;
                    vertices.push(Vertex {
                        position: [p.x, p.y, p.z],
                        color: [0.7, 0.8, 0.9],
                        uv: mesh.uv[index as usize],
                        layer,
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
            assert_eq!(textured_meshes, if neighborhood { 32 } else { 6 });
            renderer.set_textures(&device, &queue, Some(&TextureSet::from_assets(&assets)));
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
                .filter(|pixel| {
                    pixel[0].abs_diff(126) > 4
                        || pixel[1].abs_diff(160) > 4
                        || pixel[2].abs_diff(177) > 4
                })
                .count();
            assert!(drawn > 10, "Expected real geometry pixels, got {drawn}");
            drop(data);
            readback.unmap();
            assert!(device.pop_error_scope().await.is_none());
            eprintln!(
                "GPU rendered {} meshes / {} triangles into {drawn} sample pixels ({textured_meshes} meshes with linked diffuse maps)",
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
            let mut pattern = vec![255; 64 * 64 * 4];
            for y in 0..64 {
                for x in 0..64 {
                    let i = (y * 64 + x) * 4;
                    pattern[i..i + 4].copy_from_slice(if x < 32 && y < 32 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0, 0, 255, 255]
                    });
                }
            }
            let mut texture_layers = vec![vec![255; 64 * 64 * 4]; 10];
            texture_layers.push(pattern);
            renderer.set_textures(
                &device,
                &queue,
                Some(&TextureSet {
                    id: 99,
                    layers: texture_layers,
                }),
            );
            // Repeated UVs sample a red texture region; a farther green triangle
            // is deliberately submitted AFTER the textured foreground triangle.
            let mut vertices = Vec::new();
            for (z, color) in [(2., [1., 0., 0.]), (4., [0., 1., 0.])] {
                for (x, y) in [(-0.5, -0.5), (0.5, -0.5), (0., 0.5)] {
                    vertices.push(Vertex {
                        position: [x * z, y * z, z],
                        color: if z == 2.0 { [1.; 3] } else { color },
                        uv: [1.25, -0.75],
                        layer: if z == 2.0 { 10 } else { 0 },
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
