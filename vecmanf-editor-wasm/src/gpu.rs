//! Owns the `wgpu` device/surface and GPU submission (ADR 0001 §3, §4):
//! the one piece of this crate that is not plain, host-independent Rust,
//! and therefore the one piece `cargo test` cannot exercise. Takes
//! [`vecmanf_render_core::DrawList`] in, submits triangles out — no
//! editing logic lives here, matching this crate's one job.
//!
//! `wgpu` is a `wasm32`-only dependency (`Cargo.toml`): this whole module
//! only compiles for that target, so `cargo test`/`cargo clippy` on the
//! host never needs a GPU driver.

use vecmanf_document_core::ViewTransform;
use vecmanf_render_core::DrawList;
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

/// `--canvas-bg` (`docs/design-system.md`): cleared behind every frame's
/// geometry.
const CANVAS_BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0xE8 as f64 / 255.0,
    g: 0xE8 as f64 / 255.0,
    b: 0xEB as f64 / 255.0,
    a: 1.0,
};

const SHADER_SOURCE: &str = r"
struct ScreenTransform {
    scale_x: f32,
    offset_x: f32,
    scale_y: f32,
    offset_y: f32,
};

@group(0) @binding(0)
var<uniform> transform: ScreenTransform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(
        input.position.x * transform.scale_x + transform.offset_x,
        input.position.y * transform.scale_y + transform.offset_y,
        0.0,
        1.0,
    );
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
";

/// One triangle-list vertex in the shape the GPU pipeline below expects:
/// a clip-ready `f32` position (before the per-frame screen transform)
/// and a normalized `f32` color.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl From<vecmanf_render_core::Vertex> for GpuVertex {
    fn from(vertex: vecmanf_render_core::Vertex) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        let position = [vertex.position.x as f32, vertex.position.y as f32];
        let color = [
            f32::from(vertex.color.r) / 255.0,
            f32::from(vertex.color.g) / 255.0,
            f32::from(vertex.color.b) / 255.0,
            f32::from(vertex.color.a) / 255.0,
        ];
        Self { position, color }
    }
}

/// The CPU-computed per-frame mapping from document millimetres straight
/// to clip space — a document point's screen pixel position (via
/// [`ViewTransform`]) further mapped to `[-1, 1]` by the canvas's own
/// pixel size, folded into one scale-and-offset per axis so the vertex
/// shader does only a multiply-add (`specs/path-node-editing/adrs.md`:
/// this is the view transform ADR 0011 §3 shares with `ui-core`,
/// applied once, uniformly, here rather than baked into any vertex).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenTransform {
    scale_x: f32,
    offset_x: f32,
    scale_y: f32,
    offset_y: f32,
}

impl ScreenTransform {
    fn new(view: ViewTransform, canvas_width: u32, canvas_height: u32) -> Self {
        let (width, height) = (
            f64::from(canvas_width.max(1)),
            f64::from(canvas_height.max(1)),
        );
        let scale_x = 2.0 * view.scale() / width;
        let scale_y = -2.0 * view.scale() / height;
        // The document point that currently maps to screen pixel (0, 0)
        // — exactly what `ViewTransform::screen_to_document` answers.
        let origin = view.screen_to_document(0.0, 0.0);
        #[allow(clippy::cast_possible_truncation)]
        let offset_x = (-origin.x * scale_x) as f32 - 1.0;
        #[allow(clippy::cast_possible_truncation)]
        let offset_y = (-origin.y * scale_y) as f32 + 1.0;
        #[allow(clippy::cast_possible_truncation)]
        Self {
            scale_x: scale_x as f32,
            offset_x,
            scale_y: scale_y as f32,
            offset_y,
        }
    }
}

/// The `wgpu` state for one attached canvas.
pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
}

/// The buffer/bind-group pair that feeds [`ScreenTransform`] to the vertex
/// shader, plus the layout the pipeline needs to match it. Split out of
/// [`Gpu::attach`] purely to keep that function under the `clippy::
/// too_many_lines` budget — these three objects have no life of their own
/// outside `attach`.
struct TransformResources {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

fn create_transform_resources(device: &wgpu::Device) -> TransformResources {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vecmanf screen transform"),
        size: std::mem::size_of::<ScreenTransform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vecmanf transform layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vecmanf transform bind group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    TransformResources {
        buffer,
        bind_group_layout,
        bind_group,
    }
}

/// Builds the one render pipeline this crate ever submits: the draw-list
/// triangle list, transformed by the `transform_bind_group_layout` uniform,
/// targeting `surface_format`. Split out of [`Gpu::attach`] for the same
/// `clippy::too_many_lines` reason as [`create_transform_resources`].
fn create_pipeline(
    device: &wgpu::Device,
    transform_bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vecmanf draw-list shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("vecmanf pipeline layout"),
        bind_group_layouts: &[Some(transform_bind_group_layout)],
        immediate_size: 0,
    });

    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("vecmanf draw-list pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(vertex_layout)],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// The surface, its backing adapter-derived device/queue, and the
/// configuration currently applied to the surface. Split out of
/// [`Gpu::attach`] for the same `clippy::too_many_lines` reason as
/// [`create_transform_resources`].
struct SurfaceAndDevice {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

async fn create_surface_and_device(
    canvas: HtmlCanvasElement,
    width: u32,
    height: u32,
) -> Result<SurfaceAndDevice, JsValue> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|err| JsValue::from_str(&format!("creating the wgpu surface failed: {err}")))?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..wgpu::RequestAdapterOptions::default()
        })
        .await
        .map_err(|err| JsValue::from_str(&format!("no suitable GPU adapter: {err}")))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|err| JsValue::from_str(&format!("requesting the GPU device failed: {err}")))?;

    let mut config = surface
        .get_default_config(&adapter, width.max(1), height.max(1))
        .ok_or_else(|| JsValue::from_str("the surface has no usable default configuration"))?;
    config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    surface.configure(&device, &config);

    Ok(SurfaceAndDevice {
        surface,
        device,
        queue,
        config,
    })
}

impl Gpu {
    /// Creates the `wgpu` device/surface for `canvas`, sized
    /// `width`×`height` CSS pixels.
    ///
    /// # Errors
    /// Returns a `JsValue` error (a plain string) if no adapter/device
    /// could be obtained, or the surface has no usable default
    /// configuration — this is reported to the host as an `attach`
    /// failure, not a panic: a webview without WebGL2 is a real,
    /// user-facing condition (ADR 0001's own measured prerequisite), not
    /// a programming error.
    pub async fn attach(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<Self, JsValue> {
        let SurfaceAndDevice {
            surface,
            device,
            queue,
            config,
        } = create_surface_and_device(canvas, width, height).await?;

        let TransformResources {
            buffer: transform_buffer,
            bind_group_layout: transform_bind_group_layout,
            bind_group: transform_bind_group,
        } = create_transform_resources(&device);

        let pipeline = create_pipeline(&device, &transform_bind_group_layout, config.format);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            transform_buffer,
            transform_bind_group,
        })
    }

    /// Reconfigures the surface to `width`×`height` — acceptance: the
    /// canvas layer reconfigures the `wgpu` surface on every resize,
    /// before the next render (`specs/path-node-editing/adrs.md`'s PASS
    /// note, requirement 2). A stale surface composites at the wrong
    /// size; this is a correctness fix, not the measured teardown
    /// segfault, which is unrelated and outside this crate's control.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Uploads `draw_list` and submits one frame, using `view` to build
    /// this frame's screen transform (the canvas's current pixel size is
    /// `self.config.width`/`height`, kept in sync by [`Gpu::resize`]).
    ///
    /// # Errors
    /// Returns a `JsValue` error if the surface's current texture could
    /// not be acquired (e.g. the surface was lost) — reported to the
    /// host rather than panicking, since a dropped frame should not
    /// crash the editor.
    pub fn render(&mut self, draw_list: &DrawList, view: ViewTransform) -> Result<(), JsValue> {
        let transform = ScreenTransform::new(view, self.config.width, self.config.height);
        self.queue
            .write_buffer(&self.transform_buffer, 0, bytemuck::bytes_of(&transform));

        let vertices: Vec<GpuVertex> = draw_list
            .triangles
            .iter()
            .copied()
            .map(GpuVertex::from)
            .collect();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            other => {
                return Err(JsValue::from_str(&format!(
                    "acquiring the surface texture failed: {other:?}"
                )));
            }
        };
        let view_texture = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vecmanf frame encoder"),
            });

        // An empty document still needs its one clear pass below, or the
        // canvas would show whatever the GPU happened to leave behind in
        // this texture rather than `--canvas-bg` — so the vertex buffer
        // (and the draw call using it) is only created/issued when there
        // is anything to draw, but the render pass itself always runs.
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            wgpu::util::DeviceExt::create_buffer_init(
                &self.device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("vecmanf vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            )
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vecmanf draw-list pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_texture,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CANVAS_BACKGROUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(vertex_buffer) = &vertex_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.transform_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                #[allow(clippy::cast_possible_truncation)]
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}
