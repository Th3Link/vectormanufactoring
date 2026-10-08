//! The GPU pipeline's pieces: the draw-list shader, the vertex shape, the
//! per-frame screen transform, and the pipeline and multisampled target
//! built from them. Split out of `gpu.rs` so that file keeps the device,
//! surface and frame submission (`docs/technical-debt.md`, `Session` size
//! item). Compiles for `wasm32` only, like `gpu.rs`.

use curvyo_document_core::{Point, ViewTransform};

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
    @location(2) depth: f32,
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
        input.depth,
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
/// a clip-ready `f32` position (before the per-frame screen transform),
/// a normalized `f32` color and the depth of the vertex's artwork layer
/// ([`layer_depth`]).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GpuVertex {
    position: [f32; 2],
    color: [f32; 4],
    depth: f32,
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
pub(super) fn to_gpu_vertex(
    vertex: curvyo_render_core::Vertex,
    origin: Point,
    depth: f32,
) -> GpuVertex {
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
    GpuVertex {
        position,
        color,
        depth,
    }
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
pub(super) struct ScreenTransform {
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
    pub(super) fn new(view: ViewTransform, css_width: f64, css_height: f64) -> Self {
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

/// The buffer/bind-group pair that feeds [`ScreenTransform`] to the vertex
/// shader, plus the layout the pipeline needs to match it. Split out of
/// [`Gpu::attach`] purely to keep that function under the `clippy::
/// too_many_lines` budget — these three objects have no life of their own
/// outside `attach`.
pub(super) struct TransformResources {
    pub(super) buffer: wgpu::Buffer,
    pub(super) bind_group_layout: wgpu::BindGroupLayout,
    pub(super) bind_group: wgpu::BindGroup,
}

pub(super) fn create_transform_resources(device: &wgpu::Device) -> TransformResources {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curvyo screen transform"),
        size: std::mem::size_of::<ScreenTransform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("curvyo transform layout"),
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
        label: Some("curvyo transform bind group"),
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

/// The depth attachment's format. 32-bit float keeps tens of thousands of
/// layers apart ([`curvyo_render_core::DrawList::vertex_depths`]).
pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// How a pipeline treats depth.
#[derive(Clone, Copy)]
pub(super) enum DepthMode {
    /// Artwork: a vertex of layer `k` passes only where nothing of its layer
    /// or a later one was written, and writes its depth. A pixel is therefore
    /// painted at most once per layer (single coverage), and later layers
    /// still paint over earlier ones.
    SingleCoverage,
    /// Overlay (editor decorations): always passes, never writes, so
    /// translucent glyphs blend in list order as they always did.
    Overlay,
}

/// The layout of [`GpuVertex`]: position, colour, layer depth.
fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
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
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 24,
                shader_location: 2,
            },
        ],
    }
}

/// Builds a render pipeline for the draw-list triangle list, transformed by
/// the `transform_bind_group_layout` uniform, targeting `surface_format`, with
/// the depth behaviour of `depth`. Split out of [`Gpu::attach`] for the same
/// `clippy::too_many_lines` reason as [`create_transform_resources`].
pub(super) fn create_pipeline(
    device: &wgpu::Device,
    transform_bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth: DepthMode,
) -> wgpu::RenderPipeline {
    let (depth_write_enabled, depth_compare) = match depth {
        DepthMode::SingleCoverage => (true, wgpu::CompareFunction::Less),
        DepthMode::Overlay => (false, wgpu::CompareFunction::Always),
    };
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("curvyo draw-list shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("curvyo pipeline layout"),
        bind_group_layouts: &[Some(transform_bind_group_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match depth {
            DepthMode::SingleCoverage => "curvyo artwork pipeline",
            DepthMode::Overlay => "curvyo overlay pipeline",
        }),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(vertex_layout())],
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
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_write_enabled),
            depth_compare: Some(depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
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
pub(super) fn create_msaa_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> Option<wgpu::TextureView> {
    if sample_count <= 1 {
        return None;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("curvyo msaa color target"),
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

/// (Re)creates the depth attachment for the current surface size, with the
/// same sample count as the color target (a render pass refuses a mismatch).
/// Must be called wherever [`create_msaa_view`] is. Roughly doubles the
/// render target memory (`docs/technical-debt.md`, "MSAA x `HiDPI` memory").
pub(super) fn create_depth_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("curvyo depth target"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: sample_count.max(1),
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
