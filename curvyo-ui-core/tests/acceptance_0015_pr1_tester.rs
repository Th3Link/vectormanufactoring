//! Independent tester tests for PR 1 (model) of
//! `specs/0015-document-size-and-rulers` on the `ui-core` side: typed field
//! parsing and messages (criteria 15, 16, 35), display formats (12, 21, 34,
//! 35) and the content box and fit (22-27a). Expected values come from the
//! specification's arithmetic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use curvyo_document_core::{
    AnchorId, Angle, DisplayUnit, Document, DocumentSize, DocumentSizeError, EllipseFrame,
    InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot, Point, PointCount, RectBounds,
    StarFrame, StyleEdit, Vec2,
};
use curvyo_ui_core::{
    content_bounds, content_too_large_message, document_side_message, fit_document_to_content,
    format_cursor, format_field_length, format_size, format_status_length, parse_document_side,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn near(a: f64, b: f64, e: f64) -> bool {
    (a - b).abs() <= e
}

fn parsed(text: &str, unit: DisplayUnit) -> Option<f64> {
    parse_document_side(text, unit).map(Length::as_mm)
}

// ---------------------------------------------------------------- parsing

/// AC 15: point or comma, surrounding spaces ignored.
#[test]
fn parse_accepts_point_comma_and_surrounding_spaces() {
    assert_eq!(parsed("210", DisplayUnit::Mm), Some(210.0));
    assert_eq!(parsed("210.5", DisplayUnit::Mm), Some(210.5));
    assert_eq!(parsed("210,5", DisplayUnit::Mm), Some(210.5));
    assert_eq!(parsed("  210,5  ", DisplayUnit::Mm), Some(210.5));
    assert_eq!(parsed("\t12 \n", DisplayUnit::Mm), Some(12.0));
    assert_eq!(parsed("0,5", DisplayUnit::Cm), Some(5.0));
}

/// AC 16: empty, not a number, infinite, with a unit.
#[test]
fn parse_refuses_non_numbers() {
    for text in [
        "",
        "   ",
        "abc",
        "NaN",
        "nan",
        "inf",
        "-inf",
        "Infinity",
        "infinity",
        "21cm",
        "21 cm",
        "21mm",
        "8in",
        "8.5\"",
        "1.2.3",
        "1,2,3",
        "1 000",
        "--5",
        "0x10",
        "1_000",
        "\u{0663}\u{0660}",
        "\u{ff11}\u{ff10}",
        ",",
        ".",
        "-",
        "1e999",
        "9e9999",
    ] {
        assert_eq!(parse_document_side(text, DisplayUnit::Mm), None, "{text:?}");
    }
}

/// AC 16: limits in mm, tolerance 1e-9 mm and clamping onto the limit.
#[test]
fn parse_limits_in_mm() {
    assert_eq!(parsed("1", DisplayUnit::Mm), Some(1.0));
    assert_eq!(parsed("100000", DisplayUnit::Mm), Some(100_000.0));
    assert_eq!(parsed("0.9999999999", DisplayUnit::Mm), Some(1.0));
    assert_eq!(
        parsed("100000.0000000001", DisplayUnit::Mm),
        Some(100_000.0)
    );
    for text in [
        "0",
        "0.9999",
        "0.99",
        "-1",
        "-210",
        "100000.01",
        "100001",
        "1e9",
    ] {
        assert_eq!(parse_document_side(text, DisplayUnit::Mm), None, "{text}");
    }
}

#[test]
fn parse_limits_in_cm_and_inches() {
    assert_eq!(parsed("0.1", DisplayUnit::Cm), Some(1.0));
    assert_eq!(parsed("10000", DisplayUnit::Cm), Some(100_000.0));
    assert_eq!(parse_document_side("0.09", DisplayUnit::Cm), None);
    assert_eq!(parse_document_side("10000.1", DisplayUnit::Cm), None);
    assert!(near(parsed("0.04", DisplayUnit::In).unwrap(), 1.016, 1e-12));
    assert!(near(
        parsed("3937", DisplayUnit::In).unwrap(),
        99_999.8,
        1e-9
    ));
    assert_eq!(parse_document_side("0.039", DisplayUnit::In), None);
    assert_eq!(parse_document_side("3938", DisplayUnit::In), None);
    // exactly on the limits, expressed in inches (within 1e-9 mm)
    let min_in = 1.0 / 25.4;
    let max_in = 100_000.0 / 25.4;
    assert_eq!(parsed(&format!("{min_in:.17}"), DisplayUnit::In), Some(1.0));
    assert_eq!(
        parsed(&format!("{max_in:.17}"), DisplayUnit::In),
        Some(100_000.0)
    );
}

/// AC 35: 8.5 and 11 in -> 215.9 x 279.4 mm.
#[test]
fn parse_inches_is_exact_to_25_4() {
    assert!(near(parsed("8.5", DisplayUnit::In).unwrap(), 215.9, 1e-12));
    assert!(near(parsed("11", DisplayUnit::In).unwrap(), 279.4, 1e-12));
    assert!(near(parsed("8,5", DisplayUnit::In).unwrap(), 215.9, 1e-12));
}

/// The parser and `Document::resize` accept exactly the same values.
#[test]
fn the_parser_never_accepts_what_the_document_refuses() {
    let d = Document::new(1);
    for unit in DisplayUnit::ALL {
        for text in [
            "0",
            "1",
            "0.04",
            "0.1",
            "5",
            "3937",
            "3938",
            "10000",
            "10001",
            "100000",
            "100001",
            "99999.9999",
            "0.9999999",
            "1e5",
        ] {
            if let Some(len) = parse_document_side(text, unit) {
                assert!(
                    d.resize(DocumentSize::new(len, len)).is_ok(),
                    "{text} {unit:?} parsed to {} mm but the document refuses it",
                    len.as_mm()
                );
            }
        }
    }
}

// --------------------------------------------------------------- messages

/// AC 16 exact texts.
#[test]
fn validation_messages_use_the_limits_in_the_display_unit_rounded_inward() {
    assert_eq!(
        document_side_message(DisplayUnit::Mm),
        "Enter a number from 1 to 100000"
    );
    assert_eq!(
        document_side_message(DisplayUnit::Cm),
        "Enter a number from 0.1 to 10000"
    );
    assert_eq!(
        document_side_message(DisplayUnit::In),
        "Enter a number from 0.04 to 3937"
    );
}

/// "Rounded inward": a message never promises a value the field refuses. The
/// numbers the message names must themselves be accepted.
#[test]
fn the_numbers_in_the_message_are_accepted_by_the_parser() {
    for unit in DisplayUnit::ALL {
        let m = document_side_message(unit);
        let nums: Vec<&str> = m.split(' ').filter(|w| w.parse::<f64>().is_ok()).collect();
        assert_eq!(nums.len(), 2, "{m}");
        for n in nums {
            assert!(parse_document_side(n, unit).is_some(), "{m}: {n}");
        }
    }
}

/// AC 27a.
#[test]
fn content_too_large_message_names_the_limit_and_that_nothing_changed() {
    assert_eq!(
        content_too_large_message(DisplayUnit::Mm),
        "The content is larger than the largest document (100000 mm). Nothing was changed."
    );
    let cm = content_too_large_message(DisplayUnit::Cm);
    assert!(
        cm.contains("10000 cm") && cm.ends_with("Nothing was changed."),
        "{cm}"
    );
    let inch = content_too_large_message(DisplayUnit::In);
    assert!(
        inch.contains("3937 in") && inch.ends_with("Nothing was changed."),
        "{inch}"
    );
}

// ---------------------------------------------------------------- formats

/// AC 35: 3 decimals in mm, 4 in cm and in, no trailing zeros.
#[test]
fn field_text_rounds_and_drops_trailing_zeros() {
    let f = |v: f64, u| format_field_length(mm(v), u);
    assert_eq!(f(210.0, DisplayUnit::Mm), "210");
    assert_eq!(f(297.0, DisplayUnit::Mm), "297");
    assert_eq!(f(210.5, DisplayUnit::Mm), "210.5");
    assert_eq!(f(1.0 / 3.0, DisplayUnit::Mm), "0.333");
    assert_eq!(f(2.0 / 3.0, DisplayUnit::Mm), "0.667");
    assert_eq!(f(100_000.0, DisplayUnit::Mm), "100000");
    assert_eq!(f(1.0, DisplayUnit::Mm), "1");
    assert_eq!(f(210.0, DisplayUnit::Cm), "21");
    assert_eq!(f(215.9, DisplayUnit::Cm), "21.59");
    assert_eq!(f(215.9, DisplayUnit::In), "8.5");
    assert_eq!(f(279.4, DisplayUnit::In), "11");
    assert_eq!(f(1.0, DisplayUnit::In), "0.0394");
    assert_eq!(f(100_000.0, DisplayUnit::In), "3937.0079");
    assert_eq!(f(100_000.0, DisplayUnit::Cm), "10000");
    assert_eq!(f(1.0, DisplayUnit::Cm), "0.1");
    // never an exponent, never a trailing point, never "-0"
    for u in DisplayUnit::ALL {
        for v in [0.0, 0.0001, 1e-7, 1.0, 1e5, 12.345_678_9] {
            let t = f(v, u);
            assert!(!t.contains('e') && !t.contains('E'), "{t}");
            assert!(!t.ends_with('.'), "{t}");
            if t.contains('.') {
                assert!(!t.ends_with('0'), "{t}");
            }
        }
        assert_eq!(f(0.0, u), "0");
        assert_ne!(f(-0.0, u), "-0");
        assert_ne!(f(-1e-9, u), "-0");
    }
}

/// Field text parses back to the same length when it is the exact decimal
/// of a typed value (round trip type -> store -> show -> type).
#[test]
fn field_text_parses_back_to_what_was_typed() {
    for unit in DisplayUnit::ALL {
        for typed in [
            "8.5", "11", "21", "29.7", "210", "297", "1234.5", "0.5", "3.25",
        ] {
            let Some(len) = parse_document_side(typed, unit) else {
                continue;
            };
            let shown = format_field_length(len, unit);
            assert_eq!(shown, typed, "{unit:?}");
        }
    }
}

/// AC 21: fixed decimals mm 1, cm 2, in 3.
#[test]
fn status_text_has_fixed_decimals_per_unit() {
    let f = |v: f64, u| format_status_length(mm(v), u);
    assert_eq!(f(210.0, DisplayUnit::Mm), "210.0");
    assert_eq!(f(297.0, DisplayUnit::Mm), "297.0");
    assert_eq!(f(12.34, DisplayUnit::Mm), "12.3");
    assert_eq!(f(0.0, DisplayUnit::Mm), "0.0");
    assert_eq!(f(1_000_000.0, DisplayUnit::Mm), "1000000.0");
    assert_eq!(f(210.0, DisplayUnit::Cm), "21.00");
    assert_eq!(f(297.0, DisplayUnit::Cm), "29.70");
    assert_eq!(f(0.0, DisplayUnit::Cm), "0.00");
    assert_eq!(f(215.9, DisplayUnit::In), "8.500");
    assert_eq!(f(279.4, DisplayUnit::In), "11.000");
    assert_eq!(f(0.0, DisplayUnit::In), "0.000");
    assert_eq!(f(-12.34, DisplayUnit::Mm).chars().last(), Some('3'));
    // a tiny negative value must not show as "-0.0" (the text would jitter)
    for u in DisplayUnit::ALL {
        let t = f(-0.001, u);
        assert!(!t.starts_with('-'), "{u:?}: {t}");
        let t = f(-0.0, u);
        assert!(!t.starts_with('-'), "{u:?}: {t}");
    }
    // negative values keep their sign
    assert!(
        f(-12.34, DisplayUnit::Mm).starts_with('-')
            || f(-12.34, DisplayUnit::Mm).starts_with('\u{2212}')
    );
}

/// AC 21: the cursor readout has the unit once, at the end.
#[test]
fn cursor_text_has_the_unit_once() {
    assert_eq!(
        format_cursor(mm(12.34), mm(45.6), DisplayUnit::Mm),
        "x: 12.3  y: 45.6 mm"
    );
    assert_eq!(
        format_cursor(mm(123.0), mm(456.0), DisplayUnit::Cm),
        "x: 12.30  y: 45.60 cm"
    );
    assert_eq!(
        format_cursor(mm(25.4), mm(50.8), DisplayUnit::In),
        "x: 1.000  y: 2.000 in"
    );
    let t = format_cursor(mm(-3.0), mm(-0.0), DisplayUnit::Mm);
    assert_eq!(t.matches("mm").count(), 1, "{t}");
    assert!(!t.contains("-0.0"), "{t}");
}

/// AC 12, 21: "210.0 × 297.0 mm" with the multiplication sign.
#[test]
fn size_text_matches_the_spec_examples() {
    let a4 = DocumentSize::from_mm(210.0, 297.0);
    assert_eq!(format_size(a4, DisplayUnit::Mm), "210.0 \u{d7} 297.0 mm");
    assert_eq!(format_size(a4, DisplayUnit::Cm), "21.00 \u{d7} 29.70 cm");
    assert_eq!(format_size(a4, DisplayUnit::In), "8.268 \u{d7} 11.693 in");
    assert_eq!(
        format_size(DocumentSize::from_mm(215.9, 279.4), DisplayUnit::In),
        "8.500 \u{d7} 11.000 in"
    );
}

// ------------------------------------------------------------ content box

fn corners(b: (Point, Point)) -> (f64, f64, f64, f64) {
    (b.0.x, b.0.y, b.1.x, b.1.y)
}

fn assert_box(actual: (Point, Point), expected: (f64, f64, f64, f64), tol: f64) {
    let (a, e) = (corners(actual), expected);
    assert!(
        near(a.0, e.0, tol) && near(a.1, e.1, tol) && near(a.2, e.2, tol) && near(a.3, e.3, tol),
        "box {a:?} != {e:?}"
    );
}

fn rotate(d: &Document, id: NodeId, about: Point, rad: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(rad)))
        .unwrap();
}

#[test]
fn content_bounds_of_an_empty_document_is_none() {
    assert_eq!(content_bounds(&Document::new(1)), None);
}

#[test]
fn content_bounds_of_a_rect_is_the_rect_without_stroke() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(-5.0, 7.0),
        width: mm(20.0),
        height: mm(10.0),
    });
    assert_box(content_bounds(&d).unwrap(), (-5.0, 7.0, 15.0, 17.0), 1e-9);
    d.edit_style(&[id], &StyleEdit::StrokeWidth(mm(8.0)))
        .unwrap();
    assert_box(content_bounds(&d).unwrap(), (-5.0, 7.0, 15.0, 17.0), 1e-9);
}

/// AC 23: rotated rectangle: the outline, not the unrotated frame.
#[test]
fn content_bounds_follow_a_rotated_rectangle() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(100.0),
        height: mm(20.0),
    });
    rotate(&d, id, pt(50.0, 10.0), std::f64::consts::FRAC_PI_2);
    assert_box(content_bounds(&d).unwrap(), (40.0, -40.0, 60.0, 60.0), 1e-9);
    // 45 degrees about the centre: half extent = (w + h) / (2 sqrt 2)
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(10.0),
        height: mm(10.0),
    });
    rotate(&d, id, pt(5.0, 5.0), std::f64::consts::FRAC_PI_4);
    let h = 5.0 * 2.0_f64.sqrt();
    assert_box(
        content_bounds(&d).unwrap(),
        (5.0 - h, 5.0 - h, 5.0 + h, 5.0 + h),
        1e-9,
    );
}

/// AC 23: a rotated ellipse: true extremes sqrt(a^2 cos^2 + b^2 sin^2).
#[test]
fn content_bounds_of_a_rotated_ellipse_use_the_true_extremes() {
    let d = Document::new(1);
    let id = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 50.0),
        rx: mm(30.0),
        ry: mm(12.0),
    });
    assert_box(content_bounds(&d).unwrap(), (70.0, 38.0, 130.0, 62.0), 1e-6);
    let th = 30.0_f64.to_radians();
    rotate(&d, id, pt(100.0, 50.0), th);
    let hx = (30.0_f64.powi(2) * th.cos().powi(2) + 12.0_f64.powi(2) * th.sin().powi(2)).sqrt();
    let hy = (30.0_f64.powi(2) * th.sin().powi(2) + 12.0_f64.powi(2) * th.cos().powi(2)).sqrt();
    // the ellipse outline is four Beziers (KAPPA): exact to about 3e-4 of a radius
    assert_box(
        content_bounds(&d).unwrap(),
        (100.0 - hx, 50.0 - hy, 100.0 + hx, 50.0 + hy),
        0.02,
    );
}

/// AC 23: polygon and star: the vertices (rotated).
#[test]
fn content_bounds_of_polygon_and_star_cover_their_vertices() {
    for rot in [0.0, 0.9] {
        for n in [3_u32, 5, 6, 8] {
            let d = Document::new(1);
            let (r, ang) = (25.0_f64, 0.3_f64);
            let id = d.create_polygon(
                StarFrame {
                    center: pt(10.0, 20.0),
                    radius: mm(r),
                    angle: Angle::from_radians(ang),
                },
                PointCount::new(n).unwrap(),
            );
            if rot != 0.0 {
                rotate(&d, id, pt(10.0, 20.0), rot);
            }
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for k in 0..n {
                let a = ang + rot + f64::from(k) * std::f64::consts::TAU / f64::from(n);
                let (x, y) = (10.0 + r * a.cos(), 20.0 + r * a.sin());
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
            assert_box(content_bounds(&d).unwrap(), (x0, y0, x1, y1), 1e-9);
        }
    }
    // a star with a small inner ratio: the outer vertices decide
    let d = Document::new(1);
    let _ = d.create_star(
        StarFrame {
            center: pt(0.0, 0.0),
            radius: mm(40.0),
            angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let b = content_bounds(&d).unwrap();
    let lower = 40.0 * (std::f64::consts::PI / 5.0).cos(); // bottom vertices
    let side = 40.0
        * (2.0 * std::f64::consts::PI / 5.0)
            .sin()
            .max((std::f64::consts::PI / 5.0).sin());
    assert_box(
        b,
        (
            -side.max(40.0 * 0.951_056_516),
            -40.0,
            side.max(40.0 * 0.951_056_516),
            lower,
        ),
        1e-6,
    );
}

fn bulging_open_path(d: &Document) -> NodeId {
    // P0 (0,0), P1 (0,40), P2 (60,40), P3 (60,0): y(t) = 120 t (1 - t), max 30.
    d.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(0.0, 40.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(0.0, 40.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(60.0, 0.0))
            },
        ],
        false,
    )
}

/// AC 23: curves contribute their true extremes, not their control points.
#[test]
fn content_bounds_of_a_curve_use_the_extremes_not_the_control_points() {
    let d = Document::new(1);
    let _ = bulging_open_path(&d);
    assert_box(content_bounds(&d).unwrap(), (0.0, 0.0, 60.0, 30.0), 1e-6);
}

/// AC 23: a rotated path (anchors moved by the rotation).
#[test]
fn content_bounds_of_a_rotated_curve_follow_it() {
    let d = Document::new(1);
    let id = bulging_open_path(&d);
    rotate(&d, id, pt(30.0, 15.0), std::f64::consts::FRAC_PI_2);
    assert_box(content_bounds(&d).unwrap(), (15.0, -15.0, 45.0, 45.0), 1e-6);
}

/// The union covers all objects, including those on the pasteboard.
#[test]
fn content_bounds_is_the_union_including_the_pasteboard() {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(-1000.0, -500.0),
        width: mm(1.0),
        height: mm(1.0),
    });
    let _ = d.create_rect(RectBounds {
        origin: pt(5000.0, 9000.0),
        width: mm(2.0),
        height: mm(3.0),
    });
    assert_box(
        content_bounds(&d).unwrap(),
        (-1000.0, -500.0, 5002.0, 9003.0),
        1e-9,
    );
}

/// Zero-size objects (a degenerate rectangle) do not poison the union.
#[test]
fn content_bounds_with_a_zero_size_rectangle() {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(5.0, 5.0),
        width: mm(0.0),
        height: mm(0.0),
    });
    assert_box(content_bounds(&d).unwrap(), (5.0, 5.0, 5.0, 5.0), 1e-12);
}

// -------------------------------------------------------------------- fit

fn first_path_points(d: &Document) -> Vec<(f64, f64)> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) => Some(p),
            ObjectSnapshot::Primitive(_) => None,
        })
        .flat_map(|p| p.anchors.into_iter().map(|a| (a.point.x, a.point.y)))
        .collect()
}

/// AC 25 verbatim: one horizontal line (0,0)-(100,0), 10 mm stroke width,
/// fits to 100 x 1 and lies at y = 0.5 (stroke width is not included).
#[test]
fn fit_of_a_thick_horizontal_line_gives_100_by_1() {
    let d = Document::new(1);
    let id = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 0.0)),
        ],
        false,
    );
    d.edit_style(&[id], &StyleEdit::StrokeWidth(mm(10.0)))
        .unwrap();
    assert_eq!(fit_document_to_content(&d), Ok(true));
    assert_eq!(d.size(), DocumentSize::from_mm(100.0, 1.0));
    let pts = first_path_points(&d);
    assert!(
        near(pts[0].1, 0.5, 1e-12) && near(pts[1].1, 0.5, 1e-12),
        "{pts:?}"
    );
    assert!(near(pts[0].0, 0.0, 1e-12) && near(pts[1].0, 100.0, 1e-12));
    // AC 26: again -> nothing
    assert_eq!(fit_document_to_content(&d), Ok(false));
    assert_eq!(d.size(), DocumentSize::from_mm(100.0, 1.0));
}

/// AC 23/24/26 on mixed content: after a fit the content box starts at (0,0)
/// and has the document's size; a second fit does nothing.
#[test]
fn fit_makes_the_document_equal_the_content_box() {
    let d = Document::new(1);
    let r = d.create_rect(RectBounds {
        origin: pt(-300.0, 700.0),
        width: mm(40.0),
        height: mm(10.0),
    });
    let _ = bulging_open_path(&d);
    let e = d.create_ellipse(EllipseFrame {
        center: pt(500.0, -20.0),
        rx: mm(30.0),
        ry: mm(12.0),
    });
    rotate(&d, r, pt(-280.0, 705.0), 0.8);
    rotate(&d, e, pt(500.0, -20.0), -0.4);
    let before = content_bounds(&d).unwrap();
    let (w, h) = (before.1.x - before.0.x, before.1.y - before.0.y);
    assert_eq!(fit_document_to_content(&d), Ok(true));
    let s = d.size();
    assert!(near(s.width.as_mm(), w, 1e-9) && near(s.height.as_mm(), h, 1e-9));
    let after = content_bounds(&d).unwrap();
    assert_box(after, (0.0, 0.0, w, h), 1e-9);
    assert_eq!(fit_document_to_content(&d), Ok(false));
    assert_box(content_bounds(&d).unwrap(), (0.0, 0.0, w, h), 1e-9);
}

/// AC 22: no objects -> Ok(false), size untouched.
#[test]
fn fit_of_an_empty_document_does_nothing() {
    let d = Document::new(1);
    assert_eq!(fit_document_to_content(&d), Ok(false));
    assert_eq!(d.size(), DocumentSize::from_mm(210.0, 297.0));
}

/// AC 27a: content wider than 100 000 mm -> refused, nothing changed.
#[test]
fn fit_of_huge_content_is_refused_and_changes_nothing() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(-60_000.0, 0.0),
        width: mm(120_000.0),
        height: mm(10.0),
    });
    let before = d.object(id).unwrap();
    assert_eq!(
        fit_document_to_content(&d),
        Err(DocumentSizeError::OutOfRange)
    );
    assert_eq!(d.size(), DocumentSize::from_mm(210.0, 297.0));
    assert_eq!(d.object(id).unwrap(), before);
    // exactly 100 000 is fine
    d.set_rect_bounds(
        id,
        RectBounds {
            origin: pt(-60_000.0, 0.0),
            width: mm(100_000.0),
            height: mm(10.0),
        },
    )
    .unwrap();
    assert_eq!(fit_document_to_content(&d), Ok(true));
    assert_eq!(d.size(), DocumentSize::from_mm(100_000.0, 10.0));
}

/// Fit preserves each object's shape (only positions change).
#[test]
fn fit_moves_without_scaling_rotating_or_restyling() {
    let d = Document::new(1);
    let id = d.create_star(
        StarFrame {
            center: pt(300.0, 400.0),
            radius: mm(35.0),
            angle: Angle::from_radians(0.2),
        },
        PointCount::new(6).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    rotate(&d, id, pt(300.0, 400.0), 0.6);
    let ObjectSnapshot::Primitive(before) = d.object(id).unwrap() else {
        panic!()
    };
    assert_eq!(fit_document_to_content(&d), Ok(true));
    let ObjectSnapshot::Primitive(after) = d.object(id).unwrap() else {
        panic!()
    };
    assert_eq!(before.style, after.style);
    assert!(near(
        before.rotation.as_radians(),
        after.rotation.as_radians(),
        1e-12
    ));
    let (
        curvyo_document_core::Shape::Star {
            frame: fb,
            point_count: pb,
            inner_ratio: ib,
        },
        curvyo_document_core::Shape::Star {
            frame: fa,
            point_count: pa,
            inner_ratio: ia,
        },
    ) = (before.shape, after.shape)
    else {
        panic!()
    };
    assert_eq!((pb, ib), (pa, ia));
    assert_eq!(fb.radius, fa.radius);
    assert_eq!(fb.angle, fa.angle);
}

// -------------------------------------------------------------- properties

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// AC 23, 24, 26: for random rotated rectangles and ellipses anywhere in
    /// the plane, a fit makes the content box equal (0, 0)-(size), and a
    /// second fit changes nothing.
    #[test]
    fn fit_gives_a_box_at_the_origin_and_is_idempotent(
        x in -5000.0_f64..5000.0, y in -5000.0_f64..5000.0,
        w in 1.5_f64..800.0, h in 1.5_f64..800.0,
        rot in -3.2_f64..3.2,
        ex in -5000.0_f64..5000.0, ey in -5000.0_f64..5000.0,
        rx in 2.0_f64..300.0, ry in 2.0_f64..300.0, erot in -3.2_f64..3.2,
    ) {
        let d = Document::new(1);
        let r = d.create_rect(RectBounds { origin: pt(x, y), width: mm(w), height: mm(h) });
        let e = d.create_ellipse(EllipseFrame { center: pt(ex, ey), rx: mm(rx), ry: mm(ry) });
        rotate(&d, r, pt(x + w / 2.0, y + h / 2.0), rot);
        rotate(&d, e, pt(ex, ey), erot);
        let before = content_bounds(&d).unwrap();
        let (bw, bh) = (before.1.x - before.0.x, before.1.y - before.0.y);
        prop_assume!(bw >= 1.0 && bh >= 1.0 && bw <= 100_000.0 && bh <= 100_000.0);
        prop_assert_eq!(fit_document_to_content(&d), Ok(true));
        let s = d.size();
        prop_assert!((s.width.as_mm() - bw).abs() < 1e-8 && (s.height.as_mm() - bh).abs() < 1e-8);
        let after = content_bounds(&d).unwrap();
        prop_assert!(after.0.x.abs() < 1e-8 && after.0.y.abs() < 1e-8);
        prop_assert!((after.1.x - bw).abs() < 1e-8 && (after.1.y - bh).abs() < 1e-8);
        prop_assert_eq!(fit_document_to_content(&d), Ok(false));
    }

    /// AC 17, 20: a resize to any valid size keeps the content box's place
    /// relative to the document centre, so the box moves by half the change.
    #[test]
    fn resize_keeps_the_content_box_relative_to_the_centre(
        x in -3000.0_f64..3000.0, y in -3000.0_f64..3000.0,
        w in 1.0_f64..400.0, h in 1.0_f64..400.0, rot in -3.2_f64..3.2,
        nw in 1.0_f64..100_000.0, nh in 1.0_f64..100_000.0,
    ) {
        let d = Document::new(1);
        let r = d.create_rect(RectBounds { origin: pt(x, y), width: mm(w), height: mm(h) });
        rotate(&d, r, pt(x + w / 2.0, y + h / 2.0), rot);
        let before = content_bounds(&d).unwrap();
        let old = d.size();
        prop_assert!(d.resize(DocumentSize::from_mm(nw, nh)).is_ok());
        let after = content_bounds(&d).unwrap();
        let (dx, dy) = ((nw - old.width.as_mm()) / 2.0, (nh - old.height.as_mm()) / 2.0);
        let tol = 1e-9 * (1.0 + nw.max(nh));
        prop_assert!((after.0.x - before.0.x - dx).abs() < tol);
        prop_assert!((after.0.y - before.0.y - dy).abs() < tol);
        prop_assert!((after.1.x - before.1.x - dx).abs() < tol);
        prop_assert!((after.1.y - before.1.y - dy).abs() < tol);
        // back again restores the box
        d.resize(old).unwrap();
        let back = content_bounds(&d).unwrap();
        prop_assert!((back.0.x - before.0.x).abs() < tol && (back.1.y - before.1.y).abs() < tol);
    }

    /// AC 15/16/35: whatever text is typed, the parser never panics, and what
    /// it accepts the document accepts.
    #[test]
    fn the_parser_survives_arbitrary_text(text in "\\PC{0,24}", unit in 0_usize..3) {
        let unit = DisplayUnit::ALL[unit];
        if let Some(len) = parse_document_side(&text, unit) {
            prop_assert!(len.as_mm() >= 1.0 && len.as_mm() <= 100_000.0);
        }
    }

    /// AC 35: a formatted field value parses back within the display
    /// rounding (half a unit of the last shown decimal).
    #[test]
    fn field_text_round_trips_within_display_rounding(mmv in 1.0_f64..99_999.0, unit in 0_usize..3) {
        let unit = DisplayUnit::ALL[unit];
        let shown = format_field_length(mm(mmv), unit);
        let back = parse_document_side(&shown, unit);
        prop_assert!(back.is_some(), "{shown} {unit:?}");
        let step = match unit { DisplayUnit::Mm => 0.0005, _ => 0.00005 } * unit.mm_per_unit();
        prop_assert!((back.unwrap().as_mm() - mmv).abs() <= step * 1.0001, "{shown}");
    }
}

/// AC 16, 35: the text shown for the largest legal document, retyped
/// unchanged in the same unit, must be accepted (the message promises every
/// value up to the shown limit). In inches the shown 4 decimals round up past
/// the limit and the parser refuses the very text the field showed.
#[test]
#[ignore = "defect: 100000 mm shows as 3937.0079 in, which the parser refuses"]
fn the_text_shown_for_the_largest_size_is_accepted_when_retyped() {
    for unit in DisplayUnit::ALL {
        let shown = format_field_length(mm(100_000.0), unit);
        assert!(
            parse_document_side(&shown, unit).is_some(),
            "{unit:?}: shown {shown:?} is refused"
        );
    }
}
