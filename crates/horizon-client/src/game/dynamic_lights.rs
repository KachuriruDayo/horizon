//! Horizon Extras' dynamic lights (`horizon_client::dynamic_light`): the light sources near the
//! camera this frame. Nothing is gathered while the extra is off.
use super::{Game, native_hooks::NativeLightSource, shield};
use glam::DVec3;
use horizon_client::dynamic_light::{self, BURNING, Luminance, Source};

impl Game<'_> {
    /// Fills `dynamic_lights` for the frame: the player's and mobs' held items and fire, and
    /// glowing dropped items, the nearest [`dynamic_light::MAX`] of them.
    pub(super) fn gather_dynamic_lights(&mut self, camera: DVec3, alpha: f64) {
        let mut out = std::mem::take(&mut self.dynamic_lights);
        out.clear();
        if !self.settings.extras.dynamic_lights {
            self.dynamic_lights = out;
            return;
        }
        if self.luminance.is_none() {
            self.luminance = Some(Luminance::new(self.res.facts(), self.pack.assets.registry));
        }
        let lum = self.luminance.as_ref().expect("built above");
        let held = |item: Option<&str>| item.map_or(0, |i| lum.of(i));
        // An open flame: it goes out under water, where a lantern or a glowing block does not.
        let flame = |item: Option<&str>| {
            item.is_some_and(|i| i.ends_with("torch") && !i.contains("redstone") && lum.of(i) > 0)
        };
        let mut sources: Vec<(Source, bool)> = Vec::new();
        if let Some(p) = &self.player
            && !p.spectator
        {
            let feet = self
                .previous_position
                .lerp(p.position, self.clock.camera_fraction());
            let level = held(self.stack_in(self.selected).map(|s| &*s.item))
                .max(held(self.stack_in(shield::OFFHAND_SLOT).map(|s| &*s.item)))
                .max(if p.fire_ticks > 0 { BURNING } else { 0 });
            // About where the hands are.
            let burning = p.fire_ticks > 0;
            let main = self.stack_in(self.selected).map(|s| &*s.item);
            let off = self.stack_in(shield::OFFHAND_SLOT).map(|s| &*s.item);
            let open = burning
                || (flame(main) && held(main) == level)
                || (flame(off) && held(off) == level);
            sources.push((
                Source {
                    at: DVec3::new(feet.x, feet.y + 1.2, feet.z),
                    level,
                },
                open,
            ));
        }
        for m in self.mobs.iter().filter(|m| !m.removed) {
            let level = held(m.extra.main_hand.as_deref())
                .max(held(m.extra.off_hand.as_deref()))
                .max(if m.fire_ticks > 0 { BURNING } else { 0 });
            if level == 0 {
                continue;
            }
            let at = m.previous.lerp(m.position, alpha);
            let open = m.fire_ticks > 0
                || flame(m.extra.main_hand.as_deref())
                || flame(m.extra.off_hand.as_deref());
            sources.push((
                Source {
                    at: DVec3::new(at.x, at.y + f64::from(m.height()) * 0.6, at.z),
                    level,
                },
                open,
            ));
        }
        for item in &self.dropped {
            let level = lum
                .of(&item.stack.item)
                .max(if item.fire_ticks > 0 { BURNING } else { 0 });
            if level == 0 {
                continue;
            }
            let at = item.previous.lerp(item.position, alpha);
            let open = item.fire_ticks > 0 || flame(Some(&item.stack.item));
            sources.push((
                Source {
                    at: DVec3::new(at.x, at.y + 0.25, at.z),
                    level,
                },
                open,
            ));
        }
        // A native mod may take some out: nothing burns under water.
        let mut native: Vec<NativeLightSource> = sources
            .into_iter()
            .map(|(s, open_flame)| NativeLightSource {
                at: [s.at.x, s.at.y, s.at.z],
                level: s.level,
                open_flame,
            })
            .collect();
        self.native_light_sources(&mut native);
        let sources: Vec<Source> = native
            .into_iter()
            .map(|s| Source {
                at: DVec3::new(s.at[0], s.at[1], s.at[2]),
                level: s.level,
            })
            .collect();
        dynamic_light::select(&sources, camera, &mut out);
        self.dynamic_lights = out;
    }
}
