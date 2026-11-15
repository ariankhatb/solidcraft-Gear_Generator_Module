//! The Gear Generator's 3D view drawn on the GPU: the scene's triangles go up once per scene,
//! and each frame is a handful of draws (one per body and line group) into a texture egui shows.
//!
//! The view renders on the app's own device, from the UI code, before egui paints: its commands
//! are submitted to the queue egui's frame is submitted to after, so the texture is ready when
//! egui samples it. Where there is no device (tests, a host without wgpu), the view falls back to
//! its CPU rasteriser ([`crate::gear_view3d`]).
//!
//! The picture is the rasteriser's: the same flat shading (`0.3 + 0.7·|n·l|` on each member's
//! colour), the same orthographic camera, a depth test with larger nearer, translucent parts
//! after the solids without writing depth, and lines a little in front of what they lie on.

use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use egui_wgpu::wgpu;

/// A device the view can draw with, and the renderer its texture is shown through.
pub(crate) struct Shared {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Arc<egui::epaint::mutex::RwLock<egui_wgpu::Renderer>>,
    pipes: OnceLock<Pipes>,
}

static SHARED: Mutex<Option<Arc<Shared>>> = Mutex::new(None);

/// Draw the gear view with `rs`'s device (called when the app's GPU renderer is installed).
pub(crate) fn install(rs: &egui_wgpu::RenderState) {
    let shared = Shared::new(rs.device.clone(), rs.queue.clone(), rs.renderer.clone());
    *SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(shared);
}

/// The installed device, if any.
pub(crate) fn shared() -> Option<Arc<Shared>> {
    SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// Installs (or, with `None`, removes) the device every view draws with (tests).
#[cfg(test)]
pub(crate) fn set_shared(shared: Option<Arc<Shared>>) {
    *SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = shared;
}

impl Shared {
    /// Waits for the device to finish what it was given (tests timing the GPU's part).
    #[cfg(test)]
    pub(crate) fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    pub(crate) fn new(device: wgpu::Device, queue: wgpu::Queue, renderer: Arc<egui::epaint::mutex::RwLock<egui_wgpu::Renderer>>) -> Arc<Shared> {
        Arc::new(Shared { device, queue, renderer, pipes: OnceLock::new() })
    }
}

/// Bytes per vertex: position and normal, three f32 each.
pub(crate) const VERTEX: usize = 24;
const DRAW: u64 = 80;
const STRIDE: u64 = 256;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What a draw is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pass {
    /// Opaque triangles, depth written.
    Solid,
    /// Translucent triangles after the solids, depth tested, not written.
    Glass,
    /// Lines, depth tested, not written.
    Line,
}

/// One draw of a frame: vertices of the uploaded scene and how they are placed and coloured.
#[derive(Clone, Debug)]
pub(crate) struct Draw {
    pub pass: Pass,
    pub verts: Range<u32>,
    /// Rows giving pixel-space clip x, clip y and depth from a body-frame point: `row · (p, 1)`.
    pub rows: [[f32; 4]; 3],
    /// The light in the body's frame (w 1: shaded; 0: flat).
    pub light: [f32; 4],
    /// Gamma-space RGB (0..1) and alpha.
    pub color: [f32; 4],
}

struct Pipes {
    layout: wgpu::BindGroupLayout,
    solid: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
}

const SHADER: &str = r"
struct Draw {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    light: vec4<f32>,
    color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> d: Draw;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) shade: f32,
};

@vertex
fn vs(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>) -> Out {
    var o: Out;
    let q = vec4<f32>(p, 1.0);
    o.pos = vec4<f32>(dot(d.r0, q), dot(d.r1, q), dot(d.r2, q), 1.0);
    o.shade = mix(1.0, 0.3 + 0.7 * abs(dot(n, d.light.xyz)), d.light.w);
    return o;
}

fn linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(linear(clamp(d.color.rgb * i.shade, vec3<f32>(0.0), vec3<f32>(1.0))), d.color.a);
}
";

impl Pipes {
    fn new(device: &wgpu::Device) -> Pipes {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gear view draw"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: std::num::NonZeroU64::new(DRAW) },
                count: None,
            }],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("gear view"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gear view"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attributes = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
        ];
        let pipeline = |label: &str, topology, write: bool, compare, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[Some(wgpu::VertexBufferLayout { array_stride: VERTEX as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let solid = pipeline("gear view solid", wgpu::PrimitiveTopology::TriangleList, true, wgpu::CompareFunction::Greater, None);
        let glass = pipeline("gear view glass", wgpu::PrimitiveTopology::TriangleList, false, wgpu::CompareFunction::Greater, Some(wgpu::BlendState::ALPHA_BLENDING));
        let line = pipeline("gear view line", wgpu::PrimitiveTopology::LineList, false, wgpu::CompareFunction::GreaterEqual, None);
        Pipes { layout, solid, glass, line }
    }
}

struct Target {
    size: [u32; 2],
    color: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
}

/// The view's GPU resources: its uploaded scene, its target texture and egui's id for it.
pub(crate) struct Gpu {
    shared: Arc<Shared>,
    target: Option<Target>,
    id: Option<egui::TextureId>,
    verts: Option<(u64, wgpu::Buffer)>,
    uniforms: Option<(u64, wgpu::Buffer, wgpu::BindGroup)>,
    /// Frames drawn and the draws of the last one, for the bounded-work check.
    pub drawn: u64,
    pub uploads: u64,
    pub last_draws: usize,
}

impl Gpu {
    pub(crate) fn new(shared: Arc<Shared>) -> Gpu {
        Gpu { shared, target: None, id: None, verts: None, uniforms: None, drawn: 0, uploads: 0, last_draws: 0 }
    }

    /// Whether the scene `key` is uploaded.
    pub(crate) fn has_scene(&self, key: u64) -> bool {
        self.verts.as_ref().is_some_and(|(k, _)| *k == key)
    }

    /// Uploads the scene's vertices (`VERTEX` bytes each) as scene `key`.
    pub(crate) fn upload(&mut self, key: u64, bytes: &[u8]) {
        let device = &self.shared.device;
        let size = (bytes.len() as u64).max(VERTEX as u64).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("gear view vertices"), size, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        if !bytes.is_empty() {
            let pad = bytes.len().next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize);
            if pad == bytes.len() {
                self.shared.queue.write_buffer(&buffer, 0, bytes);
            } else {
                let mut b = bytes.to_vec();
                b.resize(pad, 0);
                self.shared.queue.write_buffer(&buffer, 0, &b);
            }
        }
        if let Some((_, old)) = self.verts.replace((key, buffer)) {
            old.destroy();
        }
        self.uploads += 1;
    }

    /// Draws `draws` over `bg` into a `w`×`h` picture; returns egui's id for it.
    pub(crate) fn render(&mut self, w: u32, h: u32, bg: egui::Color32, draws: &[Draw]) -> Option<egui::TextureId> {
        let shared = self.shared.clone();
        let device = &shared.device;
        let pipes = shared.pipes.get_or_init(|| Pipes::new(device));
        let (w, h) = (w.clamp(1, device.limits().max_texture_dimension_2d), h.clamp(1, device.limits().max_texture_dimension_2d));
        if self.target.as_ref().is_none_or(|t| t.size != [w, h]) {
            let texture = |label, format, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let color = texture("gear view", COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC);
            let depth = texture("gear view depth", DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
            let view = color.create_view(&Default::default());
            {
                let mut r = shared.renderer.write();
                match self.id {
                    Some(id) => r.update_egui_texture_from_wgpu_texture(device, &view, wgpu::FilterMode::Linear, id),
                    None => self.id = Some(r.register_native_texture(device, &view, wgpu::FilterMode::Linear)),
                }
            }
            if let Some(old) = self.target.replace(Target { size: [w, h], color, view, depth }) {
                old.color.destroy();
            }
        }
        let need = (draws.len() as u64).max(1) * STRIDE;
        if self.uniforms.as_ref().is_none_or(|(n, _, _)| *n < need) {
            let n = need.next_power_of_two().max(STRIDE * 16);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("gear view draws"), size: n, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gear view draws"),
                layout: &pipes.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: std::num::NonZeroU64::new(DRAW) }) }],
            });
            self.uniforms = Some((n, buffer, group));
        }
        let (_, ubuf, group) = self.uniforms.as_ref()?;
        let mut bytes = vec![0u8; draws.len() * STRIDE as usize];
        for (i, d) in draws.iter().enumerate() {
            let at = i * STRIDE as usize;
            let floats = d.rows.iter().flatten().chain(&d.light).chain(&d.color);
            for (k, f) in floats.enumerate() {
                bytes[at + k * 4..at + k * 4 + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        if !bytes.is_empty() {
            shared.queue.write_buffer(ubuf, 0, &bytes);
        }
        let target = self.target.as_ref()?;
        let bg = egui::Rgba::from(bg);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gear view") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gear view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: f64::from(bg.r()), g: f64::from(bg.g()), b: f64::from(bg.b()), a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((_, vbuf)) = &self.verts {
                pass.set_vertex_buffer(0, vbuf.slice(..));
                let mut current = None;
                for (i, d) in draws.iter().enumerate() {
                    if d.verts.is_empty() {
                        continue;
                    }
                    if current != Some(d.pass) {
                        pass.set_pipeline(match d.pass {
                            Pass::Solid => &pipes.solid,
                            Pass::Glass => &pipes.glass,
                            Pass::Line => &pipes.line,
                        });
                        current = Some(d.pass);
                    }
                    pass.set_bind_group(0, group, &[(i as u64 * STRIDE) as u32]);
                    pass.draw(d.verts.clone(), 0..1);
                }
            }
        }
        shared.queue.submit([encoder.finish()]);
        self.drawn += 1;
        self.last_draws = draws.len();
        self.id
    }

    /// The last picture's texture (for tests reading it back).
    #[cfg(test)]
    pub(crate) fn texture(&self) -> Option<&wgpu::Texture> {
        self.target.as_ref().map(|t| &t.color)
    }

    #[cfg(test)]
    pub(crate) fn device(&self) -> (&wgpu::Device, &wgpu::Queue) {
        (&self.shared.device, &self.shared.queue)
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            self.shared.renderer.write().free_texture(&id);
        }
        if let Some(t) = self.target.take() {
            t.color.destroy();
        }
        if let Some((_, b)) = self.verts.take() {
            b.destroy();
        }
    }
}
