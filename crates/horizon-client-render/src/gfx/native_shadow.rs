//! The sun's shadow map for Horizon's own shader packs (docs/shaders.md, `[shadow]`): the
//! terrain's depth seen from the sun, drawn into a square map around the camera before the
//! world, and read by the pack's material stage through `shadow_visibility`. It uses the
//! draw lists the renderer already builds for a shadow camera (`gfx.rs`: the sections that can
//! face the light, then the flat or instanced draws of them), and the terrain's own vertex
//! and sprite code (`wgsl_prefix.rs`) so cut-out leaves and plants cast the shape of their
//! texture. Only terrain casts; entities and translucent blocks do not.

use glam::{Mat4, Vec3};
use horizon_shader_api::manifest::Shadow as Settings;

use super::Frame;
use super::native_material::{TerrainShape, vanilla_terrain};
use super::pack_post::ShadowView;

/// The step, in degrees, the shadow light's direction moves in.
const STEP: f32 = 0.3;

/// The depth format of the map, and the material stage's `shadow_map`.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The entries around the terrain's code: the sun's matrix at group 3, then the vertex stages
/// (instanced and flat, as terrain's) and a fragment that only applies the alpha test.
const ENTRIES: &str = "
struct HorizonShadowLight {
    matrix: mat4x4<f32>,
}
@group(3) @binding(0) var<uniform> horizon_shadow_light: HorizonShadowLight;
struct HorizonShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
}
fn horizon_shadow_vertex(position: vec3<f32>, uv: vec2<f32>, layer: u32,
    origin: vec3<f32>) -> HorizonShadowOut {
    var out: HorizonShadowOut;
    out.clip = horizon_shadow_light.matrix * vec4(position + origin, 1.0);
    out.uv = uv;
    out.layer = layer;
    return out;
}
@vertex fn horizon_shadow_vs(
    @location(0) position: vec3<f32>, @location(1) uv: vec2<u32>, @location(2) layer: u32,
    @location(5) origin: vec4<f32>) -> HorizonShadowOut {
    return horizon_shadow_vertex(position, horizon_terrain_terrain_uv(uv, origin.w), layer,
        origin.xyz);
}
@vertex fn horizon_shadow_vs_flat(
    @builtin(vertex_index) vertex: u32,
    @location(0) position: vec3<f32>, @location(1) uv: vec2<u32>, @location(2) layer: u32
) -> HorizonShadowOut {
    let origin = horizon_terrain_slot_origins[horizon_terrain_block_slots[vertex >> 8u]];
    return horizon_shadow_vertex(position, horizon_terrain_terrain_uv(uv, origin.w), layer,
        origin.xyz);
}
@fragment fn horizon_shadow_fs(in: HorizonShadowOut) {
    // The sprite is sampled first, in uniform control flow. Bit 15 of the layer marks opaque
    // leaves: no cut-out, as in the world pass.
    let alpha = horizon_terrain_sprite(in.uv, in.layer & 0x7fffu).a;
    if alpha < 0.5 && (in.layer & 0x8000u) == 0u {
        discard;
    }
}
";

fn shader_source() -> String {
    format!("{}\n{ENTRIES}", vanilla_terrain())
}

/// The map, its sampler and the pipelines that draw into it.
pub struct NativeShadow {
    pub settings: Settings,
    /// Whether the pack's option lets the map be drawn at all.
    pub enabled: bool,
    /// The map's side in pixels, within the device's limit.
    pub size: u32,
    pub depth: wgpu::TextureView,
    light: wgpu::Buffer,
    light_group: wgpu::BindGroup,
    solid: wgpu::RenderPipeline,
    flat: Option<wgpu::RenderPipeline>,
}

impl NativeShadow {
    pub fn new(
        device: &wgpu::Device,
        shape: &TerrainShape<'_>,
        settings: Settings,
        enabled: bool,
    ) -> Result<Self, String> {
        let size = settings
            .resolution
            .clamp(1, device.limits().max_texture_dimension_2d);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Native shader pack shadow map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = texture.create_view(&Default::default());
        let light = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Native shader pack shadow light"),
            size: size_of::<[[f32; 4]; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Native shader pack shadow light"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<[[f32; 4]; 4]>() as u64),
                },
                count: None,
            }],
        });
        let light_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Native shader pack shadow light"),
            layout: &light_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light.as_entire_binding(),
            }],
        });

        let source = shader_source();
        let module = naga::front::wgsl::parse_str(&source)
            .map_err(|e| format!("shadow map: {}", e.emit_to_string(&source)))?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Native shader pack shadow map"),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let constants: Vec<(&str, f64)> = [
            "horizon_terrain_SPRITE_ARRAYS",
            "horizon_terrain_SPRITE_SPLIT",
        ]
        .into_iter()
        .zip(shape.sprites)
        .collect();
        let layout = |flat: Option<&wgpu::BindGroupLayout>| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Native shader pack shadow map"),
                bind_group_layouts: &[Some(shape.globals), flat, None, Some(&light_layout)],
                ..Default::default()
            })
        };
        let instance_attributes = wgpu::vertex_attr_array![5=>Float32x4];
        let vertices = wgpu::VertexBufferLayout {
            array_stride: size_of::<super::TerrainVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &super::TERRAIN_ATTRIBUTES,
        };
        let instances = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        };
        let pipeline = |layout: &wgpu::PipelineLayout,
                        vertex: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
            let options = wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Native shader pack shadow map"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vertex),
                    compilation_options: options.clone(),
                    buffers,
                },
                // Every face: the draw lists keep the quads that can face the light, and a
                // thin plant is two-sided.
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("horizon_shadow_fs"),
                    compilation_options: options,
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let solid = pipeline(
            &layout(None),
            "horizon_shadow_vs",
            &[Some(vertices.clone()), Some(instances)],
        );
        let flat = shape.flat.map(|group| {
            pipeline(
                &layout(Some(group)),
                "horizon_shadow_vs_flat",
                &[Some(vertices.clone())],
            )
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(format!("shadow map: {e}"));
        }
        Ok(Self {
            settings,
            enabled,
            size,
            depth,
            light,
            light_group,
            solid,
            flat,
        })
    }

    /// The direction toward the sun the shadow map is drawn for: `frame.sun_direction`, held
    /// still between steps (see `STEP`).
    pub fn sun(frame: &Frame) -> Vec3 {
        Self::toward(frame.sky.sun_angle)
    }

    /// The direction toward the moon the shadow map is drawn for.
    pub fn moon(frame: &Frame) -> Vec3 {
        Self::toward(frame.sky.moon_angle)
    }

    /// The sky is sampled once a tick, so a body's angle moves in 20 steps a second. The map
    /// is drawn for a direction that moves in coarser steps instead: each step re-rasterizes
    /// every shadow edge, and doing that every tick made them flicker at the tick rate. A
    /// still map shifts by a few texels at most, a few times a second.
    fn toward(angle: f32) -> Vec3 {
        let a = ((angle / STEP).round() * STEP).to_radians();
        Vec3::new(-a.sin(), a.cos(), 0.0)
    }

    /// The shadow camera for this frame, with what the draw lists need of it: the matrix from
    /// camera-relative world to the map (wgpu's clip space, depth 0..1), the direction toward
    /// the light and how far from the camera terrain is drawn. The light is the sun while it
    /// is up, then the moon while it is up and not new; `None` while neither is: nothing is
    /// shadowed then.
    pub fn view(&self, frame: &Frame) -> Option<ShadowView> {
        if !self.enabled {
            return None;
        }
        let sun = Self::sun(frame);
        let moon = Self::moon(frame);
        let new_moon = (frame.pack.day_time.div_euclid(24_000)).rem_euclid(8) == 4;
        let sun = if sun.y >= -0.1 {
            sun
        } else if moon.y >= -0.1 && !new_moon {
            moon
        } else {
            return None;
        };
        let distance = self.settings.distance;
        let camera = horizon_shaders::Shadow {
            resolution: self.size,
            distance,
            // The map's corners lie 1.42 half-sides out.
            render_mul: 1.5,
            // The camera moves in whole texels, so the shadows do not swim.
            interval: 2.0 * distance / self.size as f32,
            ..Default::default()
        };
        let (view, gl) = super::pack_uniforms::shadow_camera(sun, frame.camera, &camera);
        // GL's depth -w..w to wgpu's 0..w.
        let depth = Mat4::from_cols(
            glam::Vec4::X,
            glam::Vec4::Y,
            glam::Vec4::new(0.0, 0.0, 0.5, 0.0),
            glam::Vec4::new(0.0, 0.0, 0.5, 1.0),
        );
        Some(ShadowView {
            cull: depth * gl * view,
            light: sun,
            distance: Some(distance * camera.render_mul),
        })
    }

    /// How many blocks the map's depth range spans.
    pub fn depth_range(&self) -> f32 {
        let camera = horizon_shaders::Shadow::default();
        camera.far - camera.near
    }

    /// Begins the pass that draws the map, with the sun's `matrix` (`ShadowView::cull`). The
    /// caller binds the terrain's group 0, picks `instanced` or `flat`, and draws the shadow
    /// lists.
    pub fn begin<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        matrix: Mat4,
        flat: bool,
    ) -> Option<wgpu::RenderPass<'e>> {
        queue.write_buffer(
            &self.light,
            0,
            bytemuck::bytes_of(&matrix.to_cols_array_2d()),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Native shader pack shadow map"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(if flat {
            self.flat.as_ref()?
        } else {
            &self.solid
        });
        pass.set_bind_group(3, &self.light_group, &[]);
        Some(pass)
    }
}

/// The sampler the material stage compares depths with: bilinear, so each tap is a 2x2
/// filter of its own.
pub fn comparison_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Native shader pack shadow comparison"),
        compare: Some(wgpu::CompareFunction::LessEqual),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// A 1x1 map for packs with no `[shadow]`: the bindings are always there, and
/// `frame.shadow_enabled` tells the material not to read it.
pub fn empty_map(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Native shader pack no shadow map"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shadow_shader_validates_with_the_terrain_s_code() {
        let source = shader_source();
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        let entries: Vec<&str> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        for entry in [
            "horizon_shadow_vs",
            "horizon_shadow_vs_flat",
            "horizon_shadow_fs",
        ] {
            assert!(entries.contains(&entry), "{entry}");
        }
    }
}
