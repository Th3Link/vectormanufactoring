//! Owns the `wgpu` device/surface and GPU submission (ADR 0001 §3, §4):
//! the one piece of this crate that is not plain, host-independent Rust,
//! and therefore the one piece `cargo test` cannot exercise. Takes
//! [`curvyo_render_core::DrawList`] in, submits triangles out — no
//! editing logic lives here, matching this crate's one job.
//!
//! `wgpu` is a `wasm32`-only dependency (`Cargo.toml`): this whole module
//! only compiles for that target, so `cargo test`/`cargo clippy` on the
//! host never needs a GPU driver.

use curvyo_document_core::ViewTransform;
use curvyo_render_core::DrawList;
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::gpu_paint::{RampTexture, create_ramp_layout};
use crate::gpu_pipeline::{
    DEPTH_FORMAT, DepthMode, ScreenTransform, TransformResources, create_depth_view,
    create_msaa_view, create_pipeline, create_transform_resources, gpu_vertices,
};

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
fn choose_sample_count(
    color: wgpu::TextureFormatFeatureFlags,
    depth: wgpu::TextureFormatFeatureFlags,
) -> u32 {
    PREFERRED_SAMPLE_COUNTS
        .into_iter()
        .find(|&count| color.sample_count_supported(count) && depth.sample_count_supported(count))
        .unwrap_or(1)
}

/// The `wgpu` state for one attached canvas.
pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Draws the artwork prefix of the draw list with single coverage per
    /// layer (depth test "less", depth writes on).
    artwork_pipeline: wgpu::RenderPipeline,
    /// Draws the overlay (editor decorations): no depth test, no writes.
    overlay_pipeline: wgpu::RenderPipeline,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    /// The gradient ramps of the current frame (`gpu_paint.rs`).
    ramps: RampTexture,
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
    /// The depth attachment, with `sample_count` samples like the color
    /// target; recreated with it on every resize.
    depth_view: wgpu::TextureView,
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

/// The non-sRGB twin of `default` when the surface supports it, else
/// `default`. `get_default_config` prefers an sRGB format, which makes the
/// GPU sRGB-encode every shader output; but every colour this renderer
/// writes (the clear colour and each vertex colour) is already an sRGB
/// design-system value (`#2F6FEE`, ...), so encoding it again lifted
/// everything towards white (`#2F6FEE` showed as `#77B0F7`). A `Unorm`
/// target stores values as given and blends translucent colours in sRGB
/// space, which is also how CSS composites the design-system alpha tokens.
fn prefer_unorm_format(
    default: wgpu::TextureFormat,
    supported: &[wgpu::TextureFormat],
) -> wgpu::TextureFormat {
    let unorm = default.remove_srgb_suffix();
    if supported.contains(&unorm) {
        unorm
    } else {
        default
    }
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
    config.format = prefer_unorm_format(config.format, &surface.get_capabilities(&adapter).formats);
    surface.configure(&device, &config);

    // Queried from the real adapter rather than assumed: WebGL2
    // guarantees 4x but 8x is backend/driver-dependent, and this product
    // targets WebKitGTK on Linux, one of the weaker GL stacks among the
    // three engines ADR 0001 covers (`docs/technical-debt.md`'s canvas-
    // performance entry).
    let sample_count = choose_sample_count(
        adapter.get_texture_format_features(config.format).flags,
        adapter.get_texture_format_features(DEPTH_FORMAT).flags,
    );

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

        let ramp_layout = create_ramp_layout(&device);
        let artwork_pipeline = create_pipeline(
            &device,
            &transform_bind_group_layout,
            &ramp_layout,
            config.format,
            sample_count,
            DepthMode::SingleCoverage,
        );
        let overlay_pipeline = create_pipeline(
            &device,
            &transform_bind_group_layout,
            &ramp_layout,
            config.format,
            sample_count,
            DepthMode::Overlay,
        );
        let ramps = RampTexture::new(&device, ramp_layout);
        let msaa_view = create_msaa_view(&device, &config, sample_count);
        let depth_view = create_depth_view(&device, &config, sample_count);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            artwork_pipeline,
            overlay_pipeline,
            transform_buffer,
            transform_bind_group,
            ramps,
            msaa_view,
            depth_view,
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
        self.depth_view = create_depth_view(&self.device, &self.config, self.sample_count);
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
        let ramp_rows = self.ramps.upload(&self.device, &self.queue, draw_list);
        let vertices = gpu_vertices(draw_list, view.screen_to_document(0.0, 0.0), ramp_rows);

        let frame = self.acquire_frame()?;
        let view_texture = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("curvyo frame encoder"),
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
                    label: Some("curvyo vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            )
        });

        let color_attachment = self.color_attachment(&view_texture);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("curvyo draw-list pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(vertex_buffer) = &vertex_buffer {
                #[allow(clippy::cast_possible_truncation)]
                let (overlay_start, end) =
                    (draw_list.overlay_start() as u32, vertices.len() as u32);
                self.record_draws(&mut pass, vertex_buffer, overlay_start, end);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

impl Gpu {
    /// The surface texture to draw this frame into.
    fn acquire_frame(&self) -> Result<wgpu::SurfaceTexture, JsValue> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
            other => Err(JsValue::from_str(&format!(
                "acquiring the surface texture failed: {other:?}"
            ))),
        }
    }

    /// The pass's colour attachment. Multisampled: draw into `msaa_view`,
    /// resolved into the surface's own (single-sampled) `view_texture` at the
    /// end of the pass; that resolve is the actual anti-aliasing step.
    /// `Discard`: nothing downstream ever reads the multisampled texture
    /// itself, only its resolved result. No multisampling support at all
    /// (`msaa_view` is `None`, see `choose_sample_count`): draw straight into
    /// `view_texture`, no resolve.
    fn color_attachment<'a>(
        &'a self,
        view_texture: &'a wgpu::TextureView,
    ) -> wgpu::RenderPassColorAttachment<'a> {
        self.msaa_view.as_ref().map_or(
            wgpu::RenderPassColorAttachment {
                view: view_texture,
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
                resolve_target: Some(view_texture),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CANVAS_BACKGROUND),
                    store: wgpu::StoreOp::Discard,
                },
            },
        )
    }

    /// Issues the frame's draw calls: the artwork prefix first, with single
    /// coverage per layer, then the overlay over it in list order.
    fn record_draws(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        vertex_buffer: &wgpu::Buffer,
        overlay_start: u32,
        end: u32,
    ) {
        pass.set_bind_group(0, &self.transform_bind_group, &[]);
        pass.set_bind_group(1, self.ramps.bind_group(), &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        if overlay_start > 0 {
            pass.set_pipeline(&self.artwork_pipeline);
            pass.draw(0..overlay_start, 0..1);
        }
        if end > overlay_start {
            pass.set_pipeline(&self.overlay_pipeline);
            pass.draw(overlay_start..end, 0..1);
        }
    }
}
