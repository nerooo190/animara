//! Built-in sample documents (generated procedurally — no bundled assets) and help text.

use animara_document::{
    CameraState, Color, Document, Element, ElementKind, Fill, Keyframe, LayerKind, ShapePath, Stroke, SymbolKind, TextElement, TextStyle,
    Tween, TweenKind,
};
use animara_geometry::{Affine, Path, Rect, Vec2};

pub type SampleFn = fn() -> Document;

pub const SAMPLES: &[(&str, SampleFn)] = &[
    ("Bouncing Ball (classic tweens & easing)", bouncing_ball),
    ("Shape Morph (shape tween)", shape_morph),
    ("Waving Arm (bones & IK)", waving_arm),
    ("Parallax Camera (camera & layer depth)", parallax),
    ("Interactive Buttons (scripting)", interactive),
];

fn bouncing_ball() -> Document {
    let mut doc = Document::new(1280.0, 720.0, 30.0);
    doc.name = "Bouncing Ball".into();
    let id = crate::thumbs::build_default_asset(&mut doc, "Bouncing Ball");
    let tl = &mut doc.scenes[0].timeline;
    tl.layers[0].name = "Ball".into();
    tl.layers[0].frames[0].elements.push(
        Element::new(ElementKind::Instance(animara_document::SymbolInstance::new(id))).with_transform(Affine::translate(640.0, 560.0)),
    );
    tl.layers[0].extend_to(59);
    let g = tl.add_layer("Ground", 1);
    let gl = tl.layer_mut(g).unwrap();
    gl.frames[0].elements.push(Element::shape(vec![ShapePath::filled(
        Path::rect(Rect::new(0.0, 580.0, 1280.0, 720.0)),
        Fill::solid(Color::from_rgba8(0x6a, 0xb0, 0x4c, 255)),
    )]));
    doc
}

fn shape_morph() -> Document {
    let mut doc = Document::new(800.0, 600.0, 24.0);
    doc.name = "Shape Morph".into();
    let l = &mut doc.scenes[0].timeline.layers[0];
    l.frames = vec![Keyframe::new(0, 24), Keyframe::new(24, 24), Keyframe::new(48, 1)];
    let shapes = [
        ShapePath::filled(Path::rect(Rect::new(300.0, 200.0, 500.0, 400.0)), Fill::solid(Color::from_rgba8(0xe7, 0x4c, 0x3c, 255))),
        ShapePath::filled(
            Path::star(Vec2::new(400.0, 300.0), 140.0, 5, 0.45, -std::f64::consts::FRAC_PI_2),
            Fill::solid(Color::from_rgba8(0xf1, 0xc4, 0x0f, 255)),
        ),
        ShapePath::filled(Path::ellipse(Rect::new(300.0, 200.0, 500.0, 400.0)), Fill::solid(Color::from_rgba8(0x34, 0x98, 0xdb, 255))),
    ];
    for (k, s) in l.frames.iter_mut().zip(shapes) {
        k.elements.push(Element::shape(vec![s]));
    }
    let mut t = Tween::new(TweenKind::Shape);
    t.ease = animara_document::Ease::Preset { preset: animara_document::EasePreset::SineInOut };
    l.frames[0].tween = Some(t.clone());
    l.frames[1].tween = Some(t);
    doc
}

fn waving_arm() -> Document {
    let mut doc = Document::new(800.0, 600.0, 24.0);
    doc.name = "Waving Arm".into();
    let id = crate::thumbs::build_default_asset(&mut doc, "Waving Arm (rig)");
    let l = &mut doc.scenes[0].timeline.layers[0];
    l.frames[0].elements.push(
        Element::new(ElementKind::Instance(animara_document::SymbolInstance::new(id)))
            .with_transform(Affine::translate(300.0, 400.0) * Affine::scale(2.0, 2.0)),
    );
    l.extend_to(47);
    doc
}

fn parallax() -> Document {
    let mut doc = Document::new(1280.0, 720.0, 30.0);
    doc.name = "Parallax Camera".into();
    doc.background = Color::from_rgba8(0xbf, 0xe3, 0xf7, 255);
    let tl = &mut doc.scenes[0].timeline;
    tl.layers.clear();
    let mk = |name: &str, depth: f64, color: Color, y: f64, h: f64, n: usize, len: u32| {
        let mut l = animara_document::Layer::new(name);
        l.depth = depth;
        l.frames = vec![Keyframe::new(0, len)];
        for i in 0..n {
            let x = i as f64 * 400.0 - 400.0;
            let mut p = Path::new();
            p.move_to(Vec2::new(x, y + h)).line_to(Vec2::new(x + 200.0, y)).line_to(Vec2::new(x + 400.0, y + h)).close();
            l.frames[0].elements.push(Element::shape(vec![ShapePath::filled(p, Fill::solid(color))]));
        }
        l
    };
    let mut cam = animara_document::Layer::new("Camera").with_kind(LayerKind::Camera);
    cam.frames = vec![Keyframe::new(0, 89), Keyframe::new(89, 1)];
    cam.frames[0].camera = Some(CameraState::centered(1280.0, 720.0));
    cam.frames[1].camera = Some(CameraState { x: 1400.0, ..CameraState::centered(1280.0, 720.0) });
    cam.frames[0].tween = Some(Tween::new(TweenKind::Classic));
    tl.layers.push(cam);
    tl.layers.push(mk("Near hills", 0.0, Color::from_rgba8(0x2e, 0x7d, 0x32, 255), 450.0, 270.0, 8, 90));
    tl.layers.push(mk("Mid hills", 150.0, Color::from_rgba8(0x66, 0xbb, 0x6a, 255), 380.0, 340.0, 8, 90));
    tl.layers.push(mk("Far mountains", 600.0, Color::from_rgba8(0x90, 0xa4, 0xae, 255), 250.0, 470.0, 8, 90));
    doc
}

fn interactive() -> Document {
    let mut doc = Document::new(800.0, 450.0, 24.0);
    doc.name = "Interactive Buttons".into();
    let btn = crate::thumbs::build_component(&mut doc, "button");
    let tl = &mut doc.scenes[0].timeline;
    let l = &mut tl.layers[0];
    l.name = "Content".into();
    l.frames = vec![Keyframe::new(0, 1), Keyframe::new(1, 1)];
    let text = |s: &str| {
        Element::new(ElementKind::Text(TextElement::new(
            s,
            TextStyle { size: 40.0, color: Color::from_rgba8(30, 30, 30, 255), ..TextStyle::default() },
        )))
        .with_transform(Affine::translate(250.0, 120.0))
    };
    l.frames[0].elements.push(text("Page one"));
    l.frames[1].elements.push(text("Page two"));
    l.frames[1].elements.push(Element::shape(vec![ShapePath::stroked(
        Path::ellipse(Rect::new(560.0, 100.0, 640.0, 180.0)),
        Stroke::solid(Color::from_rgba8(0xe7, 0x4c, 0x3c, 255), 6.0),
    )]));
    let ui_id = tl.add_layer("Buttons", 0);
    let ul = tl.layer_mut(ui_id).unwrap();
    ul.frames = vec![Keyframe::new(0, 2)];
    let mut e =
        Element::new(ElementKind::Instance(animara_document::SymbolInstance::new(btn))).with_transform(Affine::translate(400.0, 330.0));
    if let Some(i) = e.as_instance_mut() {
        i.instance_name = "next_btn".into();
    }
    ul.frames[0].elements.push(e);
    let al = tl.add_layer("Actions", 0);
    let a = tl.layer_mut(al).unwrap();
    a.frames = vec![Keyframe::new(0, 1), Keyframe::new(1, 1)];
    a.frames[0].script = "stop();\non_click(\"next_btn\", || {\n    if currentFrame() == 1 { gotoAndStop(2); } else { gotoAndStop(1); }\n    trace(\"now on frame \" + currentFrame());\n});\n".into();
    a.frames[1].script = "stop();".into();
    let _ = SymbolKind::Button;
    doc
}

pub const HELP_TEXT: &str = r#"ANIMARA QUICK REFERENCE

Workspace
  Tools (left) · Stage (center) · Timeline (bottom) · Properties / Library / Color … (right)
  Panels can be dragged, docked, tabbed and floated. Window ▸ Workspaces saves layouts.
  Ctrl+Shift+P opens the command palette; every command is searchable.

Drawing
  V Selection   A Subselection   Q Free Transform   F Gradient Transform   L Lasso
  B Paint Brush (pressure/velocity)   Shift+B Classic Brush   Shift+Y Pencil   E Eraser
  R Rectangle   O Oval   N Line   P Pen   T Text   K Paint Bucket   S Ink Bottle
  I Eyedropper   W Asset Warp   M Bone   C Camera   H Hand   Z Zoom
  Hold Space or use the middle mouse button to pan. Ctrl+wheel zooms.
  Shift constrains (lines to 45°, squares/circles, proportional scaling, 15° rotation).
  Alt draws from the center; Alt-drag with Selection duplicates.

Timeline
  F5 frame · Shift+F5 remove frames · F6 keyframe · Shift+F6 clear keyframe · F7 blank keyframe
  Enter play/stop · , and . step · Ctrl+, Ctrl+. previous/next keyframe
  Right-click frames to create Classic, Motion or Shape tweens; drag keyframes to retime.
  Right-click layer names for Guide / Mask / Masked / motion guides / parenting / properties.
  The Auto Keyframe button makes edits on tweened frames create keyframes automatically.

Symbols
  F8 converts the selection to a symbol; double-click an instance to edit it in place.
  Graphic symbols follow the parent timeline (loop / play once / single frame).
  Movie clips play independently in Test Movie; Buttons use Up/Over/Down/Hit frames.

Scripting (Window ▸ Actions, F9)
  Frame scripts use Rhai with an ActionScript-like API:
    stop(); play(); gotoAndPlay(10); gotoAndStop("label"); nextFrame(); prevFrame();
    clip("name").stop(); clip("name").set_x(100.0); trace("hello");
    on_click("button_name", || { gotoAndStop(2); });
    on_enter_frame("name", || { ... }); on_key("ArrowRight", || { nextFrame(); });
  Control ▸ Test Movie (Ctrl+Enter) runs scripts, buttons and sounds.

Import / Export
  Open: .anim (native), .fla (CS5+ ZIP), .xfl, .svg, images.
  Import: images, animated GIF, SVG, audio (WAV; MP3/OGG/FLAC via FFmpeg), video (via FFmpeg).
  Export: PNG / PNG sequence / GIF / APNG / WebP / MP4 / WebM / MOV / SVG / sprite sheet /
          Lottie JSON / HTML5 Canvas / SWF / XFL / FLA.
"#;
