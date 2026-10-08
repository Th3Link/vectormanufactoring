//! Benchmark for `stroke-and-fill-styling` PR 1 (`specs/0007-stroke-and-fill-
//! styling/adrs.md`, readiness check section 5): the cost of reading every
//! object's style at rest, with the 200-object document of
//! `unified_object_editing.rs`'s benchmark, once with default styles and once
//! with every style key set. If a frame at rest passes 25 ms, the draw-list
//! cache moves into PR 2. Run in release by hand:
//! `cargo test --release -p curvyo-editor-wasm --test style_read_cost -- --ignored --nocapture`.

#![allow(clippy::unwrap_used)]

use curvyo_document_core::{
    AnchorId, Color, DashPattern, Document, FillMode, FillModeTarget, GradientStop, Length,
    LineCap, LineJoin, NewAnchor, NodeId, Point, RectBounds, StopId, StyleEdit, pack,
};
use curvyo_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn document_with_200_objects() -> (Document, Vec<NodeId>) {
    let document = Document::new(1);
    let mut ids = Vec::new();
    for i in 0..100u32 {
        let (x, y) = (f64::from(i % 10) * 25.0, f64::from(i / 10) * 25.0);
        ids.push(document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        }));
    }
    for i in 0..100u32 {
        let (cx, cy) = (
            f64::from(i % 10) * 25.0 + 5.0,
            400.0 + f64::from(i / 10) * 25.0,
        );
        let anchors: Vec<NewAnchor> = (0..50u32)
            .map(|n| {
                let a = f64::from(n) / 50.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * 50 + u64::from(n)),
                    pt(cx + 8.0 * a.cos(), cy + 8.0 * a.sin()),
                )
            })
            .collect();
        ids.push(document.create_path(&anchors, true));
    }
    (document, ids)
}

fn style_everything(document: &Document, ids: &[NodeId]) {
    let red = Color { r: 255, g: 0, b: 0 };
    for (n, id) in ids.iter().enumerate() {
        let edit = |edit: StyleEdit| document.edit_style(&[*id], &edit).unwrap();
        edit(StyleEdit::StrokeWidth(Length::from_mm(0.5)));
        edit(StyleEdit::StrokeColor(red));
        edit(StyleEdit::StrokeDash(
            DashPattern::new(vec![6.0, 4.0]).unwrap(),
        ));
        edit(StyleEdit::StrokeJoin(LineJoin::Round));
        edit(StyleEdit::StrokeCap(LineCap::Round));
        let counter = n as u64 * 2;
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id: *id,
                    seed_stops: GradientStop::default_pair(
                        red,
                        StopId::new(5, counter),
                        StopId::new(5, counter + 1),
                    )
                    .to_vec(),
                }],
            )
            .unwrap();
    }
}

fn at_rest(document: &Document) -> std::time::Duration {
    let bytes = pack(document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    let _ = session.draw_list();
    let began = std::time::Instant::now();
    for _ in 0..10 {
        let _ = session.draw_list();
    }
    began.elapsed() / 10
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn a_200_object_frame_at_rest_with_default_and_with_full_styles() {
    let (document, ids) = document_with_200_objects();
    println!(
        "at rest, default styles: {:?} per frame",
        at_rest(&document)
    );
    style_everything(&document, &ids);
    println!(
        "at rest, every key set:  {:?} per frame",
        at_rest(&document)
    );
}
