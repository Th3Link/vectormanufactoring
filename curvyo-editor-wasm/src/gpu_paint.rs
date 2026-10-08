//! The gradient ramp texture: one row of `RAMP_TEXELS` RGBA8 texels per gradient
//! fill of the frame, sampled by the fragment shader with clamp-to-edge, which
//! is SVG's "pad" spread (`specs/0007-stroke-and-fill-styling` criteria 21, 22).
//! Split from `gpu_pipeline.rs` (the shader and the vertex shape) so each file
//! has one job. Compiles for `wasm32` only, like `gpu.rs`.

// Every `as u32` below is of `RAMP_TEXELS` (256) or of a row count of at most
// `MAX_GRADIENTS` (1024).
#![allow(clippy::cast_possible_truncation)]

use curvyo_render_core::{DrawList, RAMP_TEXELS};

/// The ramp texture's format: unorm, so the sRGB-encoded texels are read back
/// as they were written and filtering interpolates the encoded values.
const RAMP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The bind group layout of the ramp texture and its sampler (group 1, used by
/// the fragment shader).
pub(super) fn create_ramp_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("curvyo ramp layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// The ramp texture of one `Gpu`, with the bind group that reaches it. It has
/// at least one row (a frame without gradients still binds something) and
/// grows by powers of two, never shrinks, so a document that adds and removes
/// a gradient does not reallocate every frame.
pub(super) struct RampTexture {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    rows: u32,
}

impl RampTexture {
    pub(super) fn new(device: &wgpu::Device, layout: wgpu::BindGroupLayout) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("curvyo ramp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let (texture, bind_group) = Self::allocate(device, &layout, &sampler, 1);
        Self {
            layout,
            sampler,
            texture,
            bind_group,
            rows: 1,
        }
    }

    fn allocate(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        rows: u32,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("curvyo ramp texture"),
            size: wgpu::Extent3d {
                width: RAMP_TEXELS as u32,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: RAMP_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curvyo ramp bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (texture, bind_group)
    }

    /// Uploads this frame's ramps (one row per painted gradient of
    /// `draw_list`, in order) and returns the texture's row count, which the
    /// per-vertex attributes of [`DrawList::gradient_attributes`] are built for.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_list: &DrawList,
    ) -> usize {
        let gradients = draw_list.painted_gradients();
        // At most `MAX_GRADIENTS` (1024), so the row count fits a `u32`.
        let needed = u32::try_from(gradients.len().next_power_of_two()).unwrap_or(u32::MAX);
        if needed > self.rows {
            let (texture, bind_group) = Self::allocate(device, &self.layout, &self.sampler, needed);
            self.texture = texture;
            self.bind_group = bind_group;
            self.rows = needed;
        }
        if !gradients.is_empty() {
            let texels: Vec<u8> = gradients
                .iter()
                .flat_map(|fill| fill.ramp.0.iter().flatten().copied())
                .collect();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(RAMP_TEXELS as u32 * 4),
                    rows_per_image: Some(gradients.len() as u32),
                },
                wgpu::Extent3d {
                    width: RAMP_TEXELS as u32,
                    height: gradients.len() as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.rows as usize
    }

    pub(super) const fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
