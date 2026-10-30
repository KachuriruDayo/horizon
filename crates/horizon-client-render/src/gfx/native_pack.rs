//! One of Horizon's own shader packs while it runs (docs/shaders.md): the frame its stages
//! read, its sky, material and post stages, and the timer of their passes.

use horizon_shader_api::{Report, StageKind};

use super::native_material::{NativeMaterial, TerrainShape};
use super::native_post::{NativeFrame, NativePost};
use super::native_shadow::NativeShadow;
use super::native_sky::NativeSky;
use super::pack_timing::PassTiming;

pub struct NativePack {
    pub frame: NativeFrame,
    pub sky: Option<NativeSky>,
    pub material: Option<NativeMaterial>,
    pub post: Option<NativePost>,
    /// The sun's shadow map, when the pack asks for one (`[shadow]`) and has a material.
    pub shadow: Option<NativeShadow>,
    /// GPU time of each pass, while the renderer asks for it (`pack_timing.rs`).
    pub timing: Option<PassTiming>,
}

impl NativePack {
    /// The pack of the checked stages `report` with the option values `constants`, drawing
    /// onto the world's targets as `terrain` describes; the sky reads the terrain's group 0.
    /// Also returns its stages' labels, in the order they draw. Errors name the stage wgpu
    /// refused.
    pub fn new(
        device: &wgpu::Device,
        terrain: &TerrainShape<'_>,
        report: Report,
        constants: &[(String, f64)],
    ) -> Result<(Self, Vec<String>), String> {
        let (globals, format) = (terrain.globals, terrain.format);
        let mut frame = NativeFrame::new(device);
        let shadow = report
            .shadow
            .filter(|_| report.stages.iter().any(|s| s.kind == StageKind::Material))
            .map(|settings| {
                // Drawn only while the option it follows is high enough (a low quality
                // setting leaves the second pass over the terrain out).
                let on = settings.option.as_ref().is_none_or(|id| {
                    constants
                        .iter()
                        .find(|(name, _)| name == id)
                        .is_some_and(|(_, value)| *value >= f64::from(settings.at_least))
                });
                NativeShadow::new(device, terrain, settings, on)
            })
            .transpose()?;
        frame.set_shadow_map(device, shadow.as_ref().map(|s| &s.depth));
        let (post, others): (Vec<_>, Vec<_>) = report
            .stages
            .into_iter()
            .partition(|s| s.kind == StageKind::Post);
        let sky = others
            .iter()
            .find(|s| s.kind == StageKind::Sky)
            .map(|stage| NativeSky::new(device, globals, &frame, stage, constants, format))
            .transpose()?;
        let material = others
            .iter()
            .find(|s| s.kind == StageKind::Material)
            .map(|stage| NativeMaterial::new(device, terrain, &frame, stage, constants))
            .transpose()?;
        let post = (!post.is_empty())
            .then(|| NativePost::new(device, post, constants, format, &frame))
            .transpose()?;
        let mut labels: Vec<String> = sky.iter().map(|s| s.label.clone()).collect();
        labels.extend(
            others
                .iter()
                .filter(|s| material.is_some() && s.kind == StageKind::Material)
                .map(|s| s.label.clone()),
        );
        if let Some(post) = &post {
            labels.extend(post.labels().into_iter().map(str::to_owned));
        }
        let pack = Self {
            frame,
            sky,
            material,
            post,
            shadow,
            timing: None,
        };
        Ok((pack, labels))
    }

    /// Whether the pack draws anything the game runs today.
    pub fn draws(&self) -> bool {
        self.sky.is_some() || self.material.is_some() || self.post.is_some()
    }
}
