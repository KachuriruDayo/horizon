//! Particle types (`ParticleTypes`): the vanilla particle types with a client
//! implementation, by registry name, and their per-type flags.

macro_rules! particle_types {
    ($($variant:ident = $name:literal),* $(,)?) => {
        /// Vanilla particle types with a client implementation here.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ParticleType { $($variant),* }
        impl ParticleType {
    /// The slot (0 to 63) of a native mod's own particle, `None` for any other type.
    pub fn fx_slot(self) -> Option<u8> {
        (self as u32).checked_sub(Self::HorizonFx0 as u32).filter(|&i| i < 64).map(|i| i as u8)
    }
    /// The type of a native mod's particle slot.
    pub fn fx(slot: u8) -> Option<Self> {
        Self::ALL.get(Self::HorizonFx0 as usize + usize::from(slot)).copied().filter(|t| t.fx_slot() == Some(slot))
    }
            pub const ALL: &'static [ParticleType] = &[$(ParticleType::$variant),*];
            /// Registry path (`minecraft:<name>`), also the sprite-set file name.
            pub fn name(self) -> &'static str {
                match self { $(ParticleType::$variant => $name),* }
            }
            pub fn from_name(name: &str) -> Option<Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name { $($name => Some(ParticleType::$variant),)* _ => None }
            }
        }
    };
}
particle_types! {
    Block = "block",
    BlockCrumble = "block_crumble",
    Trail = "trail",
    DustPillar = "dust_pillar",
    FallingDust = "falling_dust",
    Smoke = "smoke",
    LargeSmoke = "large_smoke",
    WhiteSmoke = "white_smoke",
    Flame = "flame",
    SoulFireFlame = "soul_fire_flame",
    CopperFireFlame = "copper_fire_flame",
    SmallFlame = "small_flame",
    Lava = "lava",
    DrippingWater = "dripping_water",
    FallingWater = "falling_water",
    DrippingLava = "dripping_lava",
    FallingLava = "falling_lava",
    LandingLava = "landing_lava",
    DrippingHoney = "dripping_honey",
    FallingHoney = "falling_honey",
    LandingHoney = "landing_honey",
    FallingNectar = "falling_nectar",
    FallingSporeBlossom = "falling_spore_blossom",
    SporeBlossomAir = "spore_blossom_air",
    DrippingObsidianTear = "dripping_obsidian_tear",
    FallingObsidianTear = "falling_obsidian_tear",
    LandingObsidianTear = "landing_obsidian_tear",
    DrippingDripstoneWater = "dripping_dripstone_water",
    FallingDripstoneWater = "falling_dripstone_water",
    DrippingDripstoneLava = "dripping_dripstone_lava",
    FallingDripstoneLava = "falling_dripstone_lava",
    Splash = "splash",
    Rain = "rain",
    Bubble = "bubble",
    SulfurBubbles = "sulfur_bubbles",
    NoxiousGas = "noxious_gas",
    NoxiousGasCloud = "noxious_gas_cloud",
    Geyser = "geyser",
    GeyserBase = "geyser_base",
    GeyserPoof = "geyser_poof",
    GeyserPlume = "geyser_plume",
    BubblePop = "bubble_pop",
    BubbleColumnUp = "bubble_column_up",
    CurrentDown = "current_down",
    Underwater = "underwater",
    Portal = "portal",
    ReversePortal = "reverse_portal",
    Enchant = "enchant",
    Nautilus = "nautilus",
    VaultConnection = "vault_connection",
    Crit = "crit",
    EnchantedHit = "enchanted_hit",
    DamageIndicator = "damage_indicator",
    Heart = "heart",
    AngryVillager = "angry_villager",
    HappyVillager = "happy_villager",
    Composter = "composter",
    Mycelium = "mycelium",
    Dolphin = "dolphin",
    EggCrack = "egg_crack",
    Poof = "poof",
    Explosion = "explosion",
    ExplosionEmitter = "explosion_emitter",
    CampfireCosySmoke = "campfire_cosy_smoke",
    CampfireSignalSmoke = "campfire_signal_smoke",
    Note = "note",
    Dust = "dust",
    TotemOfUndying = "totem_of_undying",
    Firework = "firework",
    EndRod = "end_rod",
    Ash = "ash",
    WhiteAsh = "white_ash",
    CrimsonSpore = "crimson_spore",
    WarpedSpore = "warped_spore",
    CherryLeaves = "cherry_leaves",
    PaleOakLeaves = "pale_oak_leaves",
    TintedLeaves = "tinted_leaves",
    Effect = "effect",
    InstantEffect = "instant_effect",
    EntityEffect = "entity_effect",
    Witch = "witch",
    Infested = "infested",
    Cloud = "cloud",
    Sneeze = "sneeze",
    Snowflake = "snowflake",
    Soul = "soul",
    SculkSoul = "sculk_soul",
    DragonBreath = "dragon_breath",
    Gust = "gust",
    SmallGust = "small_gust",
    GustEmitterLarge = "gust_emitter_large",
    GustEmitterSmall = "gust_emitter_small",
    DustColorTransition = "dust_color_transition",
    SculkCharge = "sculk_charge",
    SculkChargePop = "sculk_charge_pop",
    Shriek = "shriek",
    SonicBoom = "sonic_boom",
    Vibration = "vibration",
    SweepAttack = "sweep_attack",
    Glow = "glow",
    WaxOn = "wax_on",
    WaxOff = "wax_off",
    ElectricSpark = "electric_spark",
    Scrape = "scrape",
    SquidInk = "squid_ink",
    GlowSquidInk = "glow_squid_ink",
    Item = "item",
    ItemSlime = "item_slime",
    ItemSnowball = "item_snowball",
    ItemCobweb = "item_cobweb",
    SulfurCubeGoo = "sulfur_cube_goo",
    Flash = "flash",
    Firefly = "firefly",
    TrialSpawnerDetection = "trial_spawner_detection",
    TrialSpawnerDetectionOminous = "trial_spawner_detection_ominous",
    TrialOmen = "trial_omen",
    RaidOmen = "raid_omen",
    OminousSpawning = "ominous_spawning",
    Spit = "spit",
    Fishing = "fishing",
    DustPlume = "dust_plume",
    RedPoplarLeaves = "red_poplar_leaves",
    OrangePoplarLeaves = "orange_poplar_leaves",
    YellowPoplarLeaves = "yellow_poplar_leaves",
    PauseMobGrowth = "pause_mob_growth",
    ResetMobGrowth = "reset_mob_growth",
    BlockMarker = "block_marker",
    // Horizon's own (effects.rs): not in the protocol registry, never sent.
    HorizonCascade = "horizon_cascade",
    HorizonSpray = "horizon_spray",
    HorizonRipple = "horizon_ripple",
    HorizonSplashRing = "horizon_splash_ring",
    HorizonSplash = "horizon_splash",
    HorizonSplashFoam = "horizon_splash_foam",
    HorizonWave = "horizon_wave",
    HorizonWake = "horizon_wake",
    HorizonFx0 = "horizon_fx_0",
    HorizonFx1 = "horizon_fx_1",
    HorizonFx2 = "horizon_fx_2",
    HorizonFx3 = "horizon_fx_3",
    HorizonFx4 = "horizon_fx_4",
    HorizonFx5 = "horizon_fx_5",
    HorizonFx6 = "horizon_fx_6",
    HorizonFx7 = "horizon_fx_7",
    HorizonFx8 = "horizon_fx_8",
    HorizonFx9 = "horizon_fx_9",
    HorizonFx10 = "horizon_fx_10",
    HorizonFx11 = "horizon_fx_11",
    HorizonFx12 = "horizon_fx_12",
    HorizonFx13 = "horizon_fx_13",
    HorizonFx14 = "horizon_fx_14",
    HorizonFx15 = "horizon_fx_15",
    HorizonFx16 = "horizon_fx_16",
    HorizonFx17 = "horizon_fx_17",
    HorizonFx18 = "horizon_fx_18",
    HorizonFx19 = "horizon_fx_19",
    HorizonFx20 = "horizon_fx_20",
    HorizonFx21 = "horizon_fx_21",
    HorizonFx22 = "horizon_fx_22",
    HorizonFx23 = "horizon_fx_23",
    HorizonFx24 = "horizon_fx_24",
    HorizonFx25 = "horizon_fx_25",
    HorizonFx26 = "horizon_fx_26",
    HorizonFx27 = "horizon_fx_27",
    HorizonFx28 = "horizon_fx_28",
    HorizonFx29 = "horizon_fx_29",
    HorizonFx30 = "horizon_fx_30",
    HorizonFx31 = "horizon_fx_31",
    HorizonFx32 = "horizon_fx_32",
    HorizonFx33 = "horizon_fx_33",
    HorizonFx34 = "horizon_fx_34",
    HorizonFx35 = "horizon_fx_35",
    HorizonFx36 = "horizon_fx_36",
    HorizonFx37 = "horizon_fx_37",
    HorizonFx38 = "horizon_fx_38",
    HorizonFx39 = "horizon_fx_39",
    HorizonFx40 = "horizon_fx_40",
    HorizonFx41 = "horizon_fx_41",
    HorizonFx42 = "horizon_fx_42",
    HorizonFx43 = "horizon_fx_43",
    HorizonFx44 = "horizon_fx_44",
    HorizonFx45 = "horizon_fx_45",
    HorizonFx46 = "horizon_fx_46",
    HorizonFx47 = "horizon_fx_47",
    HorizonFx48 = "horizon_fx_48",
    HorizonFx49 = "horizon_fx_49",
    HorizonFx50 = "horizon_fx_50",
    HorizonFx51 = "horizon_fx_51",
    HorizonFx52 = "horizon_fx_52",
    HorizonFx53 = "horizon_fx_53",
    HorizonFx54 = "horizon_fx_54",
    HorizonFx55 = "horizon_fx_55",
    HorizonFx56 = "horizon_fx_56",
    HorizonFx57 = "horizon_fx_57",
    HorizonFx58 = "horizon_fx_58",
    HorizonFx59 = "horizon_fx_59",
    HorizonFx60 = "horizon_fx_60",
    HorizonFx61 = "horizon_fx_61",
    HorizonFx62 = "horizon_fx_62",
    HorizonFx63 = "horizon_fx_63",
    // Particles a native mod describes itself (`native_fx`): sixty-four slots, each with its own sprite set.
}
impl ParticleType {
    /// `ParticleType.getOverrideLimiter` (`ParticleTypes.register(name, true)`):
    /// spawned regardless of distance.
    pub fn override_limiter(self) -> bool {
        matches!(
            self,
            Self::BlockMarker
                | Self::Geyser
                | Self::GeyserBase
                | Self::GeyserPoof
                | Self::GeyserPlume
                | Self::DamageIndicator
                | Self::ExplosionEmitter
                | Self::Explosion
                | Self::Gust
                | Self::GustEmitterLarge
                | Self::GustEmitterSmall
                | Self::SonicBoom
                | Self::SculkCharge
                | Self::SculkChargePop
                | Self::Vibration
                | Self::Poof
                | Self::Spit
                | Self::SquidInk
                | Self::SweepAttack
                | Self::CampfireCosySmoke
                | Self::CampfireSignalSmoke
                | Self::GlowSquidInk
                | Self::Glow
                | Self::WaxOn
                | Self::WaxOff
                | Self::ElectricSpark
                | Self::Scrape
                | Self::TrialSpawnerDetection
                | Self::TrialSpawnerDetectionOminous
                | Self::VaultConnection
                | Self::OminousSpawning
        )
    }
    /// `SingleQuadParticle.Layer.translucent`: types whose vanilla particle class
    /// hardcodes the translucent layer, drawn after translucent terrain.
    /// `TerrainParticle`, `BreakingItemParticle` and `BlockMarker` resolve their
    /// layer from the sprite instead (`Layer.bySprite`); see the `translucent`
    /// sprite set on the engine.
    pub fn translucent_layer(self) -> bool {
        match self {
            // CampfireSmokeParticle.
            Self::CampfireCosySmoke | Self::CampfireSignalSmoke
            // PlayerCloudParticle.
            | Self::Cloud | Self::Sneeze
            // SpellParticle.
            | Self::Effect | Self::InstantEffect | Self::EntityEffect
            | Self::Witch | Self::Infested | Self::TrialOmen | Self::RaidOmen
            // NoxiousGasParticle.
            | Self::NoxiousGas
            // EmissiveRisingParticle.
            | Self::Soul | Self::SculkSoul
            // SculkChargeParticle, SculkChargePopParticle, ShriekParticle,
            // VibrationSignalParticle, FireflyParticle, OverlayParticle.
            | Self::SculkCharge | Self::SculkChargePop | Self::Shriek
            | Self::Vibration | Self::Firefly | Self::Flash
            // Horizon's waterfall effects.
            | Self::HorizonCascade | Self::HorizonSpray | Self::HorizonRipple
            | Self::HorizonSplashRing | Self::HorizonSplash | Self::HorizonSplashFoam
            | Self::HorizonWave | Self::HorizonWake
            // SimpleAnimatedParticle and its subclasses: EndRodParticle,
            // SparkParticle (firework), SquidInkParticle, TotemParticle.
            | Self::EndRod | Self::Firework | Self::SquidInk | Self::GlowSquidInk
            | Self::TotemOfUndying
            // FlyTowardsPositionParticle: its LifetimeAlpha(0, 0.6, 0.25, 1) is not opaque.
            | Self::VaultConnection => true,
            t if t.fx_slot().is_some() => true,
            _ => false,
        }
    }
}
