//! End-to-end tests: run the `animara-cli` binary on the XFL fixture and verify every exporter writes valid output.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animara-cli"))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xfl/bouncing/bouncing.xfl")
}

fn out_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("animara-cli-it-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> String {
    let o = cli().args(args).output().expect("run cli");
    assert!(o.status.success(), "{:?} failed: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn info_and_validate_fixture() {
    let f = fixture();
    let info = run(&["info", f.to_str().unwrap()]);
    assert!(info.contains("640x360 @ 12 fps"), "{info}");
    assert!(info.contains("3 layers, 12 frames"), "{info}");
    assert!(info.contains("1 symbols"), "{info}");
    assert!(run(&["validate", f.to_str().unwrap()]).contains("valid"));
    assert!(run(&["check-scripts", f.to_str().unwrap()]).contains("all scripts compile"));
}

#[test]
fn render_frame_matches_expectations() {
    let d = out_dir();
    let png = d.join("frame6.png");
    run(&["render", fixture().to_str().unwrap(), "--frame", "6", "--out", png.to_str().unwrap()]);
    let img = image::open(&png).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (640, 360));
    // Background color from the document.
    assert_eq!(img.get_pixel(5, 100).0, [0x33, 0x66, 0x99, 255]);
    // Ground gradient band at the bottom is green.
    let g = img.get_pixel(320, 340).0;
    assert!(g[1] > g[0] && g[1] > g[2], "{g:?}");
}

#[test]
fn every_exporter_produces_output() {
    let d = out_dir();
    let f = fixture();
    let f = f.to_str().unwrap();
    for (name, fmt) in [
        ("a.gif", "gif"),
        ("a.apng", "apng"),
        ("a.svg", "svg"),
        ("a.json", "lottie"),
        ("a.html", "html"),
        ("a.swf", "swf"),
        ("sheet.png", "sprite"),
        ("a.fla", "fla"),
    ] {
        let p = d.join(name);
        run(&["export", f, "--out", p.to_str().unwrap(), "--format", fmt]);
        assert!(std::fs::metadata(&p).map(|m| m.len() > 100).unwrap_or(false), "{fmt} output missing");
    }
    run(&["export", f, "--out", d.join("seq").to_str().unwrap(), "--format", "png-seq", "--range", "1-4"]);
    assert_eq!(std::fs::read_dir(d.join("seq")).unwrap().count(), 4);
    let lottie: serde_json::Value = serde_json::from_slice(&std::fs::read(d.join("a.json")).unwrap()).unwrap();
    assert_eq!(lottie["op"], 12);
}

#[test]
fn convert_roundtrip_xfl_anim_fla() {
    let d = out_dir();
    let anim = d.join("b.anim");
    run(&["convert", fixture().to_str().unwrap(), anim.to_str().unwrap()]);
    let fla = d.join("b.fla");
    run(&["convert", anim.to_str().unwrap(), fla.to_str().unwrap()]);
    let info = run(&["info", fla.to_str().unwrap()]);
    assert!(info.contains("3 layers, 12 frames"), "{info}");
}
