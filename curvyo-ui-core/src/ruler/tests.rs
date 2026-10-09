use super::*;
use crate::{PX_PER_MM_AT_100, Viewport};

const DIGIT: f64 = 7.0;

/// A view at `percent` zoom with the document point `(x_mm, y_mm)` at
/// screen pixel (0, 0).
fn view_at(percent: f64, x_mm: f64, y_mm: f64) -> ViewTransform {
    ViewTransform::new(PX_PER_MM_AT_100 * percent / 100.0, Point::new(x_mm, y_mm))
}

fn layout(view: ViewTransform, unit: DisplayUnit) -> RulerLayout {
    ruler_layout(view, RulerAxis::Horizontal, 800.0, unit, DIGIT)
}

/// Criterion 5's examples in mm: 100 % gives 20 mm, 8000 % gives 0.2 mm,
/// 2 % gives 1000 mm.
#[test]
fn the_spec_examples_pick_the_spec_steps_in_mm() {
    for (percent, step) in [(100.0, 20.0), (8000.0, 0.2), (2.0, 1000.0)] {
        let layout = layout(view_at(percent, 0.0, 0.0), DisplayUnit::Mm);
        assert!(
            (layout.step() - step).abs() < 1e-9,
            "{percent}: {}",
            layout.step()
        );
    }
}

/// Criterion 34: at 100 % the major step is 2 cm and 0.5 in.
#[test]
fn the_major_step_at_100_percent_is_2_cm_and_half_an_inch() {
    let cm = layout(view_at(100.0, 0.0, 0.0), DisplayUnit::Cm);
    assert!((cm.step() - 2.0).abs() < 1e-9);
    let inches = layout(view_at(100.0, 0.0, 0.0), DisplayUnit::In);
    assert!((inches.step() - 0.5).abs() < 1e-9);
}

/// Criterion 5 over every zoom from 2 % to 8000 %: the step is 1, 2 or 5
/// times a power of ten, the spacing is at least 40 px, and with a view
/// near the origin (short labels) it is under 100 px.
#[test]
fn the_spacing_is_between_40_and_100_px_at_every_zoom_and_unit() {
    for unit in DisplayUnit::ALL {
        for i in 0..=200 {
            let percent = 2.0 * (8000.0_f64 / 2.0).powf(f64::from(i) / 200.0);
            let layout = layout(view_at(percent, -20.0, -20.0), unit);
            assert!(
                layout.major_px >= MIN_MAJOR_PX && layout.major_px < 100.0,
                "{unit:?} {percent}: {}",
                layout.major_px
            );
            assert!(MANTISSAS.contains(&layout.mantissa));
            assert_eq!(layout.minors.len(), (layout.majors.len() - 1) * 4);
        }
    }
}

/// A wide label raises the step: the spacing is at least the widest
/// label plus 4 px.
#[test]
fn the_spacing_leaves_room_for_the_widest_label() {
    // 100 % at 987654 mm: labels such as "987680" are six characters.
    let view = view_at(100.0, 987_654.0, 0.0);
    let layout = layout(view, DisplayUnit::Mm);
    assert!(layout.major_px >= 6.0 * DIGIT + 4.0, "{}", layout.major_px);
}

/// Criterion 6: labels are exact decimals without trailing zeros.
#[test]
fn labels_are_exact_decimals_built_from_integers() {
    assert_eq!(label_text(3, 1, -1), "0.3");
    assert_eq!(label_text(-3, 1, -1), "\u{2212}0.3");
    assert_eq!(label_text(20, 1, -1), "2");
    assert_eq!(label_text(0, 5, -3), "0");
    assert_eq!(label_text(7, 5, -2), "0.35");
    assert_eq!(label_text(3, 2, 1), "60");
    assert_eq!(label_text(-5, 1, 2), "\u{2212}500");
    assert_eq!(label_text(1, 2, -3), "0.002");
    assert_eq!(label_text(12, 5, -4), "0.006");
    assert_eq!(label_text(1_000_000, 1, 0), "1000000");
    assert_eq!(label_text(3, 1, -6), "0.000003");
}

/// Criterion 6 at every zoom: only digits, one optional point, a leading
/// minus sign; no trailing zero after a point; the tick at 0.3 reads "0.3".
#[test]
fn no_label_at_any_zoom_has_float_noise_or_trailing_zeros() {
    for unit in DisplayUnit::ALL {
        for i in 0..=100 {
            let percent = 2.0 * (4000.0_f64).powf(f64::from(i) / 100.0);
            for x in [-300.0, -1.7, 0.0, 33.3, 12_345.6] {
                for label in &layout(view_at(percent, x, x), unit).labels {
                    let text = &label.text;
                    let body = text.strip_prefix(MINUS).unwrap_or(text);
                    assert!(
                        body.chars().all(|c| c.is_ascii_digit() || c == '.'),
                        "{text}"
                    );
                    assert!(body.matches('.').count() <= 1, "{text}");
                    if body.contains('.') {
                        assert!(!body.ends_with('0') && !body.ends_with('.'), "{text}");
                    }
                }
            }
        }
    }
    let fine = layout(view_at(8000.0, 0.0, 0.0), DisplayUnit::Mm);
    assert!(
        fine.labels.iter().any(|label| label.text == "0.4"),
        "{fine:?}"
    );
}

/// Criterion 3: value 0 is at the document's top-left corner on both
/// axes, values grow right and down.
#[test]
fn the_origin_tick_is_at_the_document_corner_on_both_axes() {
    let view = view_at(100.0, -10.0, -20.0);
    let horizontal = ruler_layout(view, RulerAxis::Horizontal, 800.0, DisplayUnit::Mm, DIGIT);
    let vertical = ruler_layout(view, RulerAxis::Vertical, 600.0, DisplayUnit::Mm, DIGIT);
    let corner = view.document_to_screen(Point::new(0.0, 0.0));
    assert!((horizontal.origin_px.unwrap() - corner.0).abs() < 1e-9);
    assert!((vertical.origin_px.unwrap() - corner.1).abs() < 1e-9);
    // Growing right: the tick after 0 is further along.
    let after = horizontal.majors.iter().find(|m| m.index == 1).unwrap();
    assert!(after.px > horizontal.origin_px.unwrap());
    let after = vertical.majors.iter().find(|m| m.index == 1).unwrap();
    assert!(after.px > vertical.origin_px.unwrap());
}

/// Criterion 4: left of and above the corner the values are negative,
/// past the document's size they continue; there is no gap.
#[test]
fn the_ruler_continues_on_the_pasteboard_without_a_break() {
    let view = view_at(100.0, -150.0, -150.0);
    let layout = layout(view, DisplayUnit::Mm);
    assert!(layout.majors.iter().any(|m| m.index < 0));
    assert!(layout.labels.iter().any(|l| l.text.starts_with(MINUS)));
    let right = self::layout(view_at(100.0, 100.0, 100.0), DisplayUnit::Mm);
    // Past 210 mm (A4 width): 100 % shows 100 mm to 311 mm here.
    assert!(
        right.labels.iter().any(|l| l.text == "240"),
        "{:?}",
        right.labels
    );
    let step = layout.major_px;
    for pair in layout.majors.windows(2) {
        assert_eq!(pair[1].index, pair[0].index + 1);
        assert!((pair[1].px - pair[0].px - step).abs() < 1e-6);
    }
    let first = layout.majors.first().unwrap();
    let last = layout.majors.last().unwrap();
    assert!(first.px <= 0.0 && last.px >= 800.0);
}

/// Criterion 2: after each of 20 pan and zoom steps every major tick lies
/// where the canvas projects its document value, within 0.5 px.
#[test]
fn every_major_tick_matches_the_canvas_projection_after_pan_and_zoom() {
    let mut viewport = Viewport::new();
    viewport.resize(800.0, 600.0);
    for step in 0..20_u32 {
        match step % 4 {
            0 => viewport.pan_by_screen_delta(37.0 * f64::from(step), -23.0),
            1 => viewport.zoom_about(300.0, 200.0, 1.7),
            2 => viewport.pan_by_screen_delta(-410.0, 91.0),
            _ => viewport.zoom_about(10.0, 590.0, 0.45),
        }
        let view = viewport.view();
        for unit in DisplayUnit::ALL {
            let layout = layout(view, unit);
            for axis in [RulerAxis::Horizontal, RulerAxis::Vertical] {
                let layout = ruler_layout(view, axis, 600.0, unit, DIGIT);
                for major in &layout.majors {
                    #[allow(clippy::cast_precision_loss)]
                    let mm = major.index as f64 * layout.step() * unit.mm_per_unit();
                    let (x, y) = view.document_to_screen(Point::new(mm, mm));
                    let projected = if axis == RulerAxis::Horizontal { x } else { y };
                    assert!((major.px - projected).abs() < 0.5, "step {step}");
                }
            }
            assert!(layout.majors.len() > 1);
        }
    }
}

/// Criterion 7: at any zoom and for values up to 1,000,000 in the display
/// unit every drawn label lies inside the strip and no two labels touch.
#[test]
fn no_label_is_clipped_or_overlaps_another_up_to_a_million() {
    for unit in DisplayUnit::ALL {
        let to_mm = unit.mm_per_unit();
        for i in 0..=40 {
            let percent = 2.0 * (4000.0_f64).powf(f64::from(i) / 40.0);
            for centre in [
                -1_000_000.0,
                -999_999.9,
                -5.5,
                0.0,
                3.0,
                999_999.9,
                1_000_000.0,
            ] {
                let scale = PX_PER_MM_AT_100 * percent / 100.0;
                // The view whose strip is centred on `centre` in the unit.
                let origin = centre * to_mm - 400.0 / scale;
                let view = ViewTransform::new(scale, Point::new(origin, origin));
                for length in [90.0, 400.0, 800.0] {
                    let layout = ruler_layout(view, RulerAxis::Horizontal, length, unit, DIGIT);
                    let mut previous_end = f64::NEG_INFINITY;
                    for label in &layout.labels {
                        #[allow(clippy::cast_precision_loss)]
                        let width = label.text.chars().count() as f64 * DIGIT;
                        let start = label.tick_px + LABEL_OFFSET_PX;
                        assert!(start >= 0.0 && start + width <= length, "{label:?}");
                        assert!(start >= previous_end, "overlap at {percent} {centre}");
                        previous_end = start + width;
                    }
                }
            }
        }
    }
}

/// Criterion 7: labels wider than the spacing allows go on every second
/// tick, and ticks are never removed.
#[test]
fn labels_thin_out_to_every_second_tick_when_they_would_touch() {
    // 4.2 px per mm: the 10 mm step is 42 px, the labels "200" are 36 px
    // wide at 12 px per digit: they fit the step (40) but not with 8 px
    // between them (44).
    let view = ViewTransform::new(4.2, Point::new(0.0, 0.0));
    let layout = ruler_layout(view, RulerAxis::Horizontal, 800.0, DisplayUnit::Mm, 12.0);
    assert!((layout.step() - 10.0).abs() < 1e-9);
    assert_eq!(layout.label_every, 2);
    assert!(layout.labels.iter().all(|l| {
        let major = layout
            .majors
            .iter()
            .find(|m| (m.px - l.tick_px).abs() < 1e-9)
            .unwrap();
        major.index % 2 == 0
    }));
    assert!(
        layout
            .majors
            .windows(2)
            .all(|p| p[1].index == p[0].index + 1)
    );
}

#[test]
fn a_label_that_does_not_fit_in_the_strip_is_not_drawn() {
    // The tick at 0 sits 2 px from the strip's start: its label would
    // start at 6 px and be 7 px wide, so with a 10 px strip it is cut.
    let view = ViewTransform::new(PX_PER_MM_AT_100, Point::new(-2.0 / PX_PER_MM_AT_100, 0.0));
    let layout = ruler_layout(view, RulerAxis::Horizontal, 10.0, DisplayUnit::Mm, DIGIT);
    assert!(layout.labels.is_empty(), "{:?}", layout.labels);
    assert!(!layout.majors.is_empty(), "ticks are never removed");
}

#[test]
fn an_empty_strip_has_an_empty_layout() {
    let view = view_at(100.0, 0.0, 0.0);
    let layout = ruler_layout(view, RulerAxis::Horizontal, 0.0, DisplayUnit::Mm, DIGIT);
    assert!(layout.majors.is_empty() && layout.labels.is_empty());
}

/// A strip of infinite length returns at once, with nothing in it.
#[test]
fn an_infinite_or_huge_strip_gets_an_empty_layout() {
    for length in [f64::INFINITY, f64::NAN, 1e300] {
        let layout = ruler_layout(
            view_at(100.0, 0.0, 0.0),
            RulerAxis::Horizontal,
            length,
            DisplayUnit::Mm,
            DIGIT,
        );
        assert!(
            layout.majors.is_empty() && layout.labels.is_empty(),
            "{length}"
        );
    }
}

/// A non-finite origin along the axis, or a non-finite scale, gives an empty
/// layout, never a NaN position. (The other axis's origin is not read.)
#[test]
fn a_non_finite_view_gets_an_empty_layout() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (view, axis) in [
            (
                ViewTransform::new(PX_PER_MM_AT_100, Point::new(bad, 0.0)),
                RulerAxis::Horizontal,
            ),
            (
                ViewTransform::new(PX_PER_MM_AT_100, Point::new(0.0, bad)),
                RulerAxis::Vertical,
            ),
            (
                ViewTransform::new(bad, Point::new(0.0, 0.0)),
                RulerAxis::Horizontal,
            ),
            (
                ViewTransform::new(bad, Point::new(0.0, 0.0)),
                RulerAxis::Vertical,
            ),
        ] {
            let layout = ruler_layout(view, axis, 800.0, DisplayUnit::Mm, DIGIT);
            assert!(layout.majors.is_empty() && layout.minors.is_empty());
        }
    }
}

/// A view far outside the zoom range (a hand-built transform) neither panics
/// nor allocates without bound.
#[test]
fn a_view_outside_any_zoom_range_does_not_panic() {
    for (scale, origin) in [
        (1e-300, -1e300),
        (1e-300, 0.0),
        (1e300, 1e300),
        (1e-9, 1e12),
    ] {
        for unit in DisplayUnit::ALL {
            let view = ViewTransform::new(scale, Point::new(origin, origin));
            let layout = ruler_layout(view, RulerAxis::Horizontal, 800.0, unit, DIGIT);
            assert!(layout.majors.len() <= 4097, "{scale} {origin}");
        }
    }
}

/// The widest label counts only ticks whose label can be drawn: the strip
/// shows 961 to 999.9 mm, so the 4-digit tick at 1000 mm just off its end has
/// no label and must not push the step from 20 to 50 mm.
#[test]
fn a_tick_just_off_the_strip_does_not_widen_the_step() {
    let view = view_at(100.0, 961.0, 0.0);
    let length = (999.9 - 961.0) * PX_PER_MM_AT_100;
    let layout = ruler_layout(view, RulerAxis::Horizontal, length, DisplayUnit::Mm, 18.0);
    assert!((layout.step() - 20.0).abs() < 1e-9, "{}", layout.step());
    assert!(layout.labels.iter().any(|label| label.text == "980"));
}
