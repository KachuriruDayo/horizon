//! Schema 1 of `horizon-shaders.toml` and the `.hshader` package.
use horizon_shader_api::{MANIFEST, Manifest, OptionKind, Pack};
use std::{collections::BTreeMap, path::Path};

fn sample_dir() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/samples/schema-1"
    ))
}

fn sample_text() -> String {
    std::fs::read_to_string(sample_dir().join(MANIFEST)).unwrap()
}

fn refused(text: &str) -> String {
    Manifest::parse(text).expect_err("should be refused")
}

#[test]
fn reads_every_key_of_schema_1() {
    let m = Manifest::parse(&sample_text()).unwrap();
    assert_eq!(
        (
            m.id.as_str(),
            m.name.as_str(),
            m.version.as_str(),
            m.api.as_str()
        ),
        ("sample_pack", "Sample Pack", "1.0.0", "0.2")
    );
    assert_eq!(m.authors, ["Horizon"]);
    assert_eq!(
        m.shadow.as_ref().map(|s| (s.resolution, s.distance)),
        Some((2048, 96.0))
    );
    assert_eq!(m.license.as_deref(), Some("MIT"));
    assert_eq!(m.sky.as_ref().unwrap().file, "sky.wgsl");
    assert_eq!(
        m.material.as_ref().unwrap().name.as_deref(),
        Some("Terrain")
    );
    let post: Vec<_> = m.post.iter().map(|s| s.file.as_str()).collect();
    assert_eq!(post, ["post/grade.wgsl", "post/vignette.wgsl"]);
    assert_eq!(
        m.files(),
        [
            "common.wgsl",
            "sky.wgsl",
            "material.wgsl",
            "post/grade.wgsl",
            "post/vignette.wgsl"
        ]
    );
    let kinds: Vec<_> = m.options.iter().map(|o| (o.id.as_str(), &o.kind)).collect();
    assert_eq!(
        kinds,
        [
            (
                "saturation",
                &OptionKind::Float {
                    default: 1.1,
                    min: 0.0,
                    max: 2.0,
                    step: 0.05
                }
            ),
            ("vignette", &OptionKind::Bool { default: true }),
            (
                "samples",
                &OptionKind::Int {
                    default: 4,
                    min: 1,
                    max: 16
                }
            ),
            (
                "tonemap",
                &OptionKind::Choice {
                    values: vec!["None".into(), "Soft".into(), "Filmic".into()],
                    default: 1
                }
            ),
        ]
    );
    let types: Vec<_> = m.options.iter().map(|o| o.kind.wgsl_type()).collect();
    assert_eq!(types, ["f32", "bool", "i32", "u32"]);
    let constants: Vec<_> = m
        .options
        .iter()
        .map(|o| o.kind.default_constant())
        .collect();
    assert_eq!(constants, [f64::from(1.1f32), 1.0, 4.0, 1.0]);
    assert_eq!(m.budget.gpu_ms, Some(1.5));
}

#[test]
fn the_smallest_manifest() {
    let m = Manifest::parse(
        "schema = 1\nid = \"p\"\nname = \"P\"\nversion = \"1\"\napi = \"0.1\"\n[[post]]\nfile = \"a.wgsl\"\n",
    )
    .unwrap();
    assert!(m.sky.is_none() && m.options.is_empty() && m.budget.gpu_ms.is_none());
}

#[test]
fn refuses_what_this_build_does_not_read() {
    let text = sample_text();
    let cases = [
        (text.replace("schema = 1", "schema = 2"), "schema 2"),
        (
            text.replace("api = \"0.2\"", "api = \"0.3\""),
            "shader API 0.3",
        ),
        (
            text.replace("\"sample_pack\"", "\"Sample\""),
            "is not [a-z0-9_]",
        ),
        (
            text.replace("license = \"MIT\"", "licence = \"MIT\""),
            "unknown field",
        ),
        (
            text.replace("gpu_ms = 1.5", "gpu_ms = 40.0"),
            "budget.gpu_ms",
        ),
        (
            text.replace("resolution = 2048", "resolution = 64"),
            "shadow.resolution",
        ),
        (
            text.replace("distance = 96.0", "distance = 1000.0"),
            "shadow.distance",
        ),
        (
            text.replace("\"common.wgsl\"", "\"../common.wgsl\""),
            "not a plain path",
        ),
        (
            text.replace("\"sky.wgsl\"", "\"sky.glsl\""),
            "not a plain path to a .wgsl",
        ),
        (
            text.replace("id = \"samples\"", "id = \"vignette\""),
            "declared twice",
        ),
        (
            text.replace("id = \"samples\"", "id = \"horizon_samples\""),
            "WGSL identifier",
        ),
        (
            text.replace("default = \"Soft\"", "default = \"Hard\""),
            "one of the values",
        ),
        (
            text.replace("default = 1.1", "default = 3.0"),
            "default in [0, 2]",
        ),
        (
            text.replace("default = 4", "default = 4.5"),
            "not a 32-bit integer",
        ),
        (
            text.replace("type = \"bool\"", "type = \"color\""),
            "is not bool",
        ),
        (
            text.replace("default = true", "default = true\nmin = 0"),
            "doesn't apply",
        ),
    ];
    for (text, needle) in cases {
        let e = refused(&text);
        assert!(e.contains(needle), "{needle:?} not in {e:?}");
        assert!(e.starts_with(MANIFEST), "{e}");
    }
    let no_stage = "schema = 1\nid = \"p\"\nname = \"P\"\nversion = \"1\"\napi = \"0.1\"\n";
    assert!(refused(no_stage).contains("no stage"));
    let many = (0..9).map(|i| format!("[[post]]\nfile = \"p{i}.wgsl\"\n"));
    assert!(refused(&format!("{no_stage}{}", many.collect::<String>())).contains("9 post passes"));
}

#[test]
fn reads_the_sample_folder_and_its_package_alike() {
    let folder = Pack::read(sample_dir()).unwrap();
    assert_eq!(folder.files.len(), 7);
    assert!(
        folder
            .source("post/grade.wgsl")
            .unwrap()
            .contains("fn post")
    );
    let bytes = folder.package().unwrap();
    assert_eq!(
        bytes,
        folder.package().unwrap(),
        "packaging is reproducible"
    );
    let dir = std::env::temp_dir().join(format!("horizon-shader-api-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sample_pack.hshader");
    std::fs::write(&path, &bytes).unwrap();
    let packaged = Pack::read(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(packaged.files, folder.files);
    assert_eq!(packaged.manifest, folder.manifest);
}

#[test]
fn refuses_files_a_pack_does_not_use() {
    let base = || -> BTreeMap<String, Vec<u8>> { Pack::read(sample_dir()).unwrap().files };
    let mut files = base();
    files.insert("old.wgsl".into(), b"fn f() {}".to_vec());
    assert!(
        Pack::from_files(files)
            .unwrap_err()
            .contains("old.wgsl is not used")
    );
    let mut files = base();
    files.insert("run.sh".into(), Vec::new());
    assert!(
        Pack::from_files(files)
            .unwrap_err()
            .contains("run.sh: a pack holds only")
    );
    let mut files = base();
    files.remove("sky.wgsl");
    assert!(
        Pack::from_files(files)
            .unwrap_err()
            .contains("sky.wgsl: named in")
    );
    let mut files = base();
    files.insert("sky.wgsl".into(), vec![0xff, 0xfe]);
    assert!(Pack::from_files(files).unwrap_err().contains("not UTF-8"));
    let mut files = base();
    files.insert("post/grade.wgsl".into(), vec![b' '; 300 << 10]);
    assert!(
        Pack::from_files(files)
            .unwrap_err()
            .contains("over 256 KiB")
    );
    let mut files = base();
    files.remove(MANIFEST);
    assert!(
        Pack::from_files(files)
            .unwrap_err()
            .contains("missing at the pack's root")
    );
}

#[test]
fn a_players_choices_are_saved_read_and_cycled() {
    use horizon_shader_api::{OptionKind, chosen, with_chosen};
    let saved = with_chosen("", "fresh", "quality", Some("High"));
    let saved = with_chosen(&saved, "fresh", "bloom", Some("0.3"));
    assert_eq!(saved, "fresh.quality=High;fresh.bloom=0.3");
    assert_eq!(chosen(&saved, "fresh", "quality"), Some("High"));
    assert_eq!(chosen(&saved, "other", "quality"), None);
    // Setting again replaces; None drops.
    let saved = with_chosen(&saved, "fresh", "quality", Some("Low"));
    assert_eq!(chosen(&saved, "fresh", "quality"), Some("Low"));
    assert_eq!(
        with_chosen(&saved, "fresh", "quality", None),
        "fresh.bloom=0.3"
    );

    let quality = OptionKind::Choice {
        values: vec!["Low".into(), "Medium".into(), "High".into()],
        default: 1,
    };
    assert_eq!(quality.default_text(), "Medium");
    assert_eq!(quality.constant_of("High"), Some(2.0));
    assert_eq!(quality.constant_of("Ultra"), None);
    assert_eq!(quality.cycle_text(None, false), "High");
    assert_eq!(quality.cycle_text(Some("High"), false), "Low");
    assert_eq!(quality.cycle_text(Some("Low"), true), "High");

    let on = OptionKind::Bool { default: true };
    assert_eq!(on.cycle_text(None, false), "false");
    assert_eq!(on.constant_of("false"), Some(0.0));

    let amount = OptionKind::Float {
        default: 0.5,
        min: 0.0,
        max: 1.0,
        step: 0.05,
    };
    assert_eq!(amount.cycle_text(None, false), "0.6");
    assert_eq!(amount.cycle_text(Some("1"), false), "0");
    assert_eq!(amount.cycle_text(Some("0"), true), "1");
    assert_eq!(amount.constant_of("7"), Some(1.0));
}
