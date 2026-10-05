//! Owns the `wgpu` device/surface and GPU submission (ADR 0001 §3, §4):
//! the one piece of this crate that is not plain, host-independent Rust,
//! and therefore the one piece `cargo test` cannot exercise. Takes
//! [`vecmanf_render_core::DrawList`] in, submits triangles out — no
//! editing logic lives here, matching this crate's one job.
//!
//! `wgpu` is a `wasm32`-only dependency (`Cargo.toml`): this whole module
//! only compiles for that target, so `cargo test`/`cargo clippy` on the
//! host never needs a GPU driver.

use vecmanf_document_core::{Point, ViewTransform};
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

/// The multisample count every draw-list triangle (node glyphs, handle
/// lines and every `lyon`-tessellated stroke are flat-colored polygon
/// edges with no AA of their own) is rendered at, in descending order of
/// preference — [`choose_sample_count`] picks the first one the
/// adapter's own surface format actually supports
/// (`TextureFormatFeatureFlags::supported_sample_counts`), rather than
/// assuming one. WebGL2 itself guarantees at least 4x (the spec's
/// `MAX_SAMPLES` floor), but 8x is backend/driver-dependent — some
/// downlevel GL stacks cap lower. `1` (no MSAA) is the last resort and
/// is handled as a genuinely different code path ([`Gpu::render`] skips
/// the offscreen multisampled target entirely when `sample_count == 1`,
/// since `wgpu` refuses a multisampled texture with `sample_count: 1`).
///
/// 4x was this slice's first fix (a real improvement over the previous
/// `count: 1` bug — no multisampling at all — which is what made some
/// axis-aligned strokes outright invisible). 8x is this one's: a real,
/// if smaller, further improvement when the adapter supports it, because
/// MSAA only resolves *edge coverage* at a fixed number of sample
/// positions per pixel — it has a ceiling this crate's technique cannot
/// cross no matter the count (and 8 is the practical ceiling for this
/// `wgpu`/GL stack specifically, not just this slice's own choice — see
/// the `WebGPU note` below). Inkscape's Cairo backend uses an analytic/
/// coverage-based software rasterizer, computed on a much finer subpixel
/// grid than any fixed sample count — not literally unlimited, but far
/// more coverage levels than 8x MSAA samples — which is why it stays
/// visibly crisper even at the same logical line width; matching that
/// exactly would mean a different rendering technique (supersampling, or
/// a shader-side analytic/distance-based edge fringe — both named, with
/// their trade-offs, in `docs/technical-debt.md`), which is a follow-up
/// story, not this fix.
///
/// **WebGPU note:** this crate's `wgpu::Instance` only requests
/// `Backends::GL` today, so this 8x finding is specific to that backend/
/// adapter. The WebGPU spec itself guarantees only `sampleCount` 1 and 4
/// everywhere; anything above 4x is adapter-optional, gated behind the
/// `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` feature. Adding
/// `Backends::BROWSER_WEBGPU` as a target later needs this reconsidered —
/// either cap the preference list at 4x on that backend, or request
/// `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` explicitly in
/// `request_device` and keep querying as today — not assumed to keep
/// working unexamined.
const PREFERRED_SAMPLE_COUNTS: [u32; 2] = [8, 4];

/// Picks the first of [`PREFERRED_SAMPLE_COUNTS`] `flags` (the surface
/// format's own [`wgpu::TextureFormatFeatureFlags`], from
/// `adapter.get_texture_format_features`) actually supports, or `1` (no
/// MSAA) if none of them are — the genuinely-unsupported-anywhere case,
/// kept as a real fallback rather than an assumed-unreachable default,
/// since a downlevel/software GL stack is exactly the kind of adapter
/// this product's `WebKitGTK` target can hand back.
fn choose_sample_count(flags: wgpu::TextureFormatFeatureFlags) -> u32 {
    PREFERRED_SAMPLE_COUNTS
        .into_iter()
        .find(|&count| flags.sample_count_supported(count))
        .unwrap_or(1)
}

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

/// Converts a draw-list vertex to the GPU's own vertex shape, **relative
/// to `origin`** (the document point currently at screen pixel `(0, 0)`)
/// — subtracted in `f64`, before the `f32` cast
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "The GPU upload
/// subtracts the view origin in `f64` before the `f32` cast"). Casting
/// an *absolute* document-mm position straight to `f32` (the previous
/// behaviour) loses precision proportional to its distance from the
/// document origin; panning far from the origin at high zoom (pan is
/// unbounded, and nothing in this spec limits it) made that loss
/// catastrophic once it approached half the stroke width. Subtracting
/// `origin` first keeps the `f32` error proportional to distance from
/// the *viewport's* origin — i.e. roughly proportional to distance on
/// screen, in pixels — regardless of where the document origin is.
/// [`ScreenTransform`]'s own offset no longer needs `origin` at all,
/// since every vertex arrives already shifted.
fn to_gpu_vertex(vertex: vecmanf_render_core::Vertex, origin: Point) -> GpuVertex {
    let relative_x = vertex.position.x - origin.x;
    let relative_y = vertex.position.y - origin.y;
    #[allow(clippy::cast_possible_truncation)]
    let position = [relative_x as f32, relative_y as f32];
    let color = [
        f32::from(vertex.color.r) / 255.0,
        f32::from(vertex.color.g) / 255.0,
        f32::from(vertex.color.b) / 255.0,
        f32::from(vertex.color.a) / 255.0,
    ];
    GpuVertex { position, color }
}

/// The CPU-computed per-frame mapping from document millimetres straight
/// to clip space — a document point's screen pixel position (via
/// [`ViewTransform`]) further mapped to `[-1, 1]` by the canvas's own
/// pixel size, folded into one scale-and-offset per axis so the vertex
/// shader does only a multiply-add (`specs/0002-path-node-editing/adrs.md`:
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
    /// `css_width`/`css_height` are the canvas's CSS (layout) pixel size
    /// — *not* its backing-buffer resolution. The two differ whenever
    /// `devicePixelRatio` is not `1` (every modern `HiDPI` display): the clip-
    /// space fraction a document point maps to depends only on where it
    /// sits within the canvas's displayed box, so computing this ratio
    /// against the (possibly DPR-scaled) physical buffer size instead
    /// would be wrong by exactly a factor of the device pixel ratio.
    /// [`Gpu::render`] is the one caller, and it is the one place the
    /// physical-vs-CSS distinction is resolved — nothing downstream of
    /// this type needs to know about `devicePixelRatio` at all.
    ///
    /// The offset is now a fixed `-1`/`+1`, not derived from `view`'s
    /// origin: every vertex [`to_gpu_vertex`] hands the GPU has already
    /// been shifted by that same origin in `f64`, so this uniform no
    /// longer needs to repeat that subtraction (`specs/0004-canvas-
    /// navigation-and-selection/adrs.md`: "the shader is unchanged; only
    /// the offset term changes").
    fn new(view: ViewTransform, css_width: f64, css_height: f64) -> Self {
        let (width, height) = (css_width.max(1.0), css_height.max(1.0));
        let scale_x = 2.0 * view.scale() / width;
        let scale_y = -2.0 * view.scale() / height;
        #[allow(clippy::cast_possible_truncation)]
        Self {
            scale_x: scale_x as f32,
            offset_x: -1.0,
            scale_y: scale_y as f32,
            offset_y: 1.0,
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
    /// The offscreen multisampled color target every frame actually
    /// renders into when `sample_count > 1`; [`Gpu::render`] resolves it
    /// down into the surface's own (single-sampled) texture. Recreated
    /// whenever the surface's size changes ([`Gpu::resize`]) — it must
    /// always match `config.width`/`height` exactly, or `wgpu` refuses
    /// the render pass. `None` when [`choose_sample_count`] found no
    /// multisampling support at all (see that function's own doc
    /// comment); [`Gpu::render`] then renders directly into the surface
    /// texture.
    msaa_view: Option<wgpu::TextureView>,
    /// The multisample count [`choose_sample_count`] chose for this
    /// adapter at attach time — fixed for the life of this `Gpu` (a
    /// resize keeps it; only the surface/`msaa_view` sizes change).
    sample_count: u32,
    /// `window.devicePixelRatio` at the last `attach`/`resize` — the
    /// surface's backing buffer is sized `css_size * device_pixel_ratio`
    /// (the host's job, see `useEditorSession.ts`'s attach effect and
    /// resize observer), so this is what lets [`Gpu::render`] recover the
    /// canvas's *CSS* pixel size (what `ScreenTransform` actually needs)
    /// from `config.width`/`height` (the physical buffer size).
    device_pixel_ratio: f64,
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
    sample_count: u32,
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
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

/// (Re)creates the offscreen multisampled color target
/// [`Gpu::render`] draws into when `sample_count > 1` — must be called
/// after every [`wgpu::Surface::configure`] that changes `config.width`/
/// `height`, since the two textures must match size exactly. `None` when
/// `sample_count` is `1`: `wgpu` refuses a "multisampled" texture with a
/// sample count of `1`, and there is nothing to resolve from in that
/// case anyway — [`Gpu::render`] renders directly into the surface
/// texture instead (see [`choose_sample_count`]'s own doc comment).
fn create_msaa_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> Option<wgpu::TextureView> {
    if sample_count <= 1 {
        return None;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vecmanf msaa color target"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    Some(texture.create_view(&wgpu::TextureViewDescriptor::default()))
}

/// The surface, its backing adapter-derived device/queue, the
/// configuration currently applied to the surface, and the multisample
/// count this specific adapter/format combination actually supports.
/// Split out of [`Gpu::attach`] for the same `clippy::too_many_lines`
/// reason as [`create_transform_resources`].
struct SurfaceAndDevice {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    sample_count: u32,
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

    // `DeviceDescriptor::default()`'s limits are the full native set,
    // which asks for things no WebGL2 backend can ever provide (e.g. a
    // nonzero `max_compute_workgroups_per_dimension` — WebGL2 has no
    // compute shaders at all) and makes `request_device` fail outright.
    // `adapter.limits()` is what this specific (downlevel) adapter
    // actually supports, which is all a flat-colored 2D draw list ever
    // needs.
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..wgpu::DeviceDescriptor::default()
        })
        .await
        .map_err(|err| JsValue::from_str(&format!("requesting the GPU device failed: {err}")))?;

    let mut config = surface
        .get_default_config(&adapter, width.max(1), height.max(1))
        .ok_or_else(|| JsValue::from_str("the surface has no usable default configuration"))?;
    config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    surface.configure(&device, &config);

    // Queried from the real adapter rather than assumed: WebGL2
    // guarantees 4x but 8x is backend/driver-dependent, and this product
    // targets WebKitGTK on Linux, one of the weaker GL stacks among the
    // three engines ADR 0001 covers (`docs/technical-debt.md`'s canvas-
    // performance entry).
    let sample_count =
        choose_sample_count(adapter.get_texture_format_features(config.format).flags);

    Ok(SurfaceAndDevice {
        surface,
        device,
        queue,
        config,
        sample_count,
    })
}

impl Gpu {
    /// Creates the `wgpu` device/surface for `canvas`. `width`×`height`
    /// are the surface's *backing-buffer* (physical) pixel size — the
    /// host sizes this `css_size * device_pixel_ratio` (see
    /// `useEditorSession.ts`'s attach effect) so the canvas's actual
    /// resolution matches a `HiDPI` display instead of being upscaled and
    /// softened; `device_pixel_ratio` is what lets [`Gpu::render`] map
    /// back to the canvas's CSS size for the clip-space transform.
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
        device_pixel_ratio: f64,
    ) -> Result<Self, JsValue> {
        let SurfaceAndDevice {
            surface,
            device,
            queue,
            config,
            sample_count,
        } = create_surface_and_device(canvas, width, height).await?;

        let TransformResources {
            buffer: transform_buffer,
            bind_group_layout: transform_bind_group_layout,
            bind_group: transform_bind_group,
        } = create_transform_resources(&device);

        let pipeline = create_pipeline(
            &device,
            &transform_bind_group_layout,
            config.format,
            sample_count,
        );
        let msaa_view = create_msaa_view(&device, &config, sample_count);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            transform_buffer,
            transform_bind_group,
            msaa_view,
            sample_count,
            device_pixel_ratio: if device_pixel_ratio > 0.0 {
                device_pixel_ratio
            } else {
                1.0
            },
        })
    }

    /// Reconfigures the surface (and its MSAA target) to `width`×`height`
    /// physical pixels and the current `device_pixel_ratio` — acceptance:
    /// the canvas layer reconfigures the `wgpu` surface on every resize,
    /// before the next render (`specs/0002-path-node-editing/adrs.md`'s PASS
    /// note, requirement 2). A stale surface composites at the wrong
    /// size; this is a correctness fix, not the measured teardown
    /// segfault, which is unrelated and outside this crate's control.
    ///
    /// This only reconfigures the surface; it does not submit a frame.
    /// [`crate::wasm_api::WasmSession::resize`] calls this and then
    /// renders immediately, in the same call, so a resize never shows a
    /// stale or empty frame while waiting for the next animation frame.
    pub fn resize(&mut self, width: u32, height: u32, device_pixel_ratio: f64) {
        let width = width.max(1);
        let height = height.max(1);
        self.device_pixel_ratio = if device_pixel_ratio > 0.0 {
            device_pixel_ratio
        } else {
            1.0
        };
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.msaa_view = create_msaa_view(&self.device, &self.config, self.sample_count);
    }

    /// Uploads `draw_list` and submits one frame, using `view` to build
    /// this frame's screen transform against the canvas's current *CSS*
    /// pixel size (`self.config.width`/`height`, the physical
    /// backing-buffer size kept in sync by [`Gpu::resize`], divided back
    /// down by `self.device_pixel_ratio`). Draws into the offscreen
    /// multisampled target (`self.sample_count`) and resolves it into
    /// the surface's own texture, which is what actually anti-aliases
    /// every stroke/glyph edge — see [`choose_sample_count`]. Renders
    /// directly into the surface texture instead, with no resolve step,
    /// on the (practically unreachable, but handled) adapter that
    /// supports no multisampling at all.
    ///
    /// # Errors
    /// Returns a `JsValue` error if the surface's current texture could
    /// not be acquired (e.g. the surface was lost) — reported to the
    /// host rather than panicking, since a dropped frame should not
    /// crash the editor.
    pub fn render(&mut self, draw_list: &DrawList, view: ViewTransform) -> Result<(), JsValue> {
        let css_width = f64::from(self.config.width) / self.device_pixel_ratio;
        let css_height = f64::from(self.config.height) / self.device_pixel_ratio;
        let transform = ScreenTransform::new(view, css_width, css_height);
        self.queue
            .write_buffer(&self.transform_buffer, 0, bytemuck::bytes_of(&transform));

        // The document point currently at screen pixel (0, 0) — every
        // vertex below is shifted by this same point in `f64`, before
        // its own `f32` cast (`to_gpu_vertex`'s own doc comment).
        let origin = view.screen_to_document(0.0, 0.0);
        let vertices: Vec<GpuVertex> = draw_list
            .triangles
            .iter()
            .copied()
            .map(|vertex| to_gpu_vertex(vertex, origin))
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

        // Multisampled: draw into `msaa_view`, resolved into the surface's
        // own (single-sampled) `view_texture` at the end of the pass —
        // that resolve is the actual anti-aliasing step. `Discard`:
        // nothing downstream ever reads the multisampled texture itself,
        // only its resolved result. No multisampling support at all
        // (`msaa_view` is `None`, see `choose_sample_count`): draw
        // straight into `view_texture`, no resolve.
        let color_attachment = self.msaa_view.as_ref().map_or(
            wgpu::RenderPassColorAttachment {
                view: &view_texture,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CANVAS_BACKGROUND),
                    store: wgpu::StoreOp::Store,
                },
            },
            |msaa_view| wgpu::RenderPassColorAttachment {
                view: msaa_view,
                depth_slice: None,
                resolve_target: Some(&view_texture),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CANVAS_BACKGROUND),
                    store: wgpu::StoreOp::Discard,
                },
            },
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vecmanf draw-list pass"),
                color_attachments: &[Some(color_attachment)],
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
