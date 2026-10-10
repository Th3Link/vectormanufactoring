//! Independent tester tests for PR 2 (rulers and pasteboard) of
//! `specs/0015-document-size-and-rulers`, `ui-core` side: the pure tick
//! layout (criteria 3 to 7) and the initial view (criterion 11a). Expected
//! values come from the specification's arithmetic and from an independent
//! integer label formatter written here, not from the implementation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use curvyo_document_core::{DisplayUnit, Point, ViewTransform};
use curvyo_ui_core::{
    DOCUMENT_INSET_PX, PX_PER_MM_AT_100, RulerAxis, RulerLayout, Viewport, ruler_layout,
};

const MINUS: char = '\u{2212}';
const UNITS: [DisplayUnit; 3] = [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::In];

fn mm_per(unit: DisplayUnit) -> f64 {
    match unit {
        DisplayUnit::Mm => 1.0,
        DisplayUnit::Cm => 10.0,
        DisplayUnit::In => 25.4,
    }
}

fn view_at(percent: f64, ox: f64, oy: f64) -> ViewTransform {
    ViewTransform::new(percent / 100.0 * PX_PER_MM_AT_100, Point::new(ox, oy))
}

/// 2 % to 8000 %, geometrically spaced, limits included.
fn zoom_sweep() -> Vec<f64> {
    let n = 200;
    (0..=n)
        .map(|i| 2.0 * (8000.0_f64 / 2.0).powf(f64::from(i) / f64::from(n)))
        .collect()
}

fn axes() -> [RulerAxis; 2] {
    [RulerAxis::Horizontal, RulerAxis::Vertical]
}

/// Independent exact decimal of `n * 10^exponent` from integers only.
fn exact_label(n: i64, exponent: i32) -> String {
    if n == 0 {
        return "0".to_owned();
    }
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let body = if exponent >= 0 {
        format!("{digits}{}", "0".repeat(exponent as usize))
    } else {
        let frac = (-exponent) as usize;
        let padded = if digits.len() <= frac {
            format!("{}{digits}", "0".repeat(frac - digits.len() + 1))
        } else {
            digits
        };
        let (int, fr) = padded.split_at(padded.len() - frac);
        let fr = fr.trim_end_matches('0');
        if fr.is_empty() {
            int.to_owned()
        } else {
            format!("{int}.{fr}")
        }
    };
    if neg { format!("{MINUS}{body}") } else { body }
}

fn label_for_major(layout: &RulerLayout, tick_px: f64) -> i64 {
    layout
        .majors
        .iter()
        .find(|m| (m.px - tick_px).abs() < 1e-6)
        .unwrap_or_else(|| panic!("label tick {tick_px} is not a major tick"))
        .index
}

fn width_of(text: &str, digit_px: f64) -> f64 {
    text.chars().count() as f64 * digit_px
}

// ---- exact label formatter self-check ----

#[test]
fn reference_formatter_is_right() {
    assert_eq!(exact_label(3, -1), "0.3");
    assert_eq!(exact_label(-3, -1), "\u{2212}0.3");
    assert_eq!(exact_label(20, 0), "20");
    assert_eq!(exact_label(2, 3), "2000");
    assert_eq!(exact_label(5, -2), "0.05");
    assert_eq!(exact_label(40, -1), "4");
    assert_eq!(exact_label(1_000_002, -1), "100000.2");
}

// ---- AC 5: step and spacing ----

#[test]
fn ac5_spec_examples_mm() {
    let cases = [
        (100.0, 2, 1, 20.0),
        (8000.0, 2, -1, 0.2),
        (2.0, 1, 3, 1000.0),
    ];
    for (pct, mant, exp, step) in cases {
        for axis in axes() {
            let l = ruler_layout(
                view_at(pct, 0.0, 0.0),
                axis,
                1000.0,
                DisplayUnit::Mm,
                0.0,
                0.0,
            );
            assert!(
                (l.step() - step).abs() < step * 1e-12,
                "pct {pct}: step {} want {step}",
                l.step()
            );
            assert_eq!((l.mantissa, l.exponent), (mant, exp), "pct {pct}");
        }
    }
}

#[test]
fn ac34_spec_examples_cm_and_in_at_100_percent() {
    let cm = ruler_layout(
        view_at(100.0, 0.0, 0.0),
        RulerAxis::Horizontal,
        1000.0,
        DisplayUnit::Cm,
        0.0,
        0.0,
    );
    assert!((cm.step() - 2.0).abs() < 1e-12, "cm step {}", cm.step());
    let inch = ruler_layout(
        view_at(100.0, 0.0, 0.0),
        RulerAxis::Horizontal,
        1000.0,
        DisplayUnit::In,
        0.0,
        0.0,
    );
    assert!((inch.step() - 0.5).abs() < 1e-12, "in step {}", inch.step());
}

#[test]
fn ac5_step_is_smallest_125_at_least_40px_without_labels() {
    for unit in UNITS {
        for pct in zoom_sweep() {
            for axis in axes() {
                let view = view_at(pct, -33.3, 71.1);
                let l = ruler_layout(view, axis, 900.0, unit, 0.0, 0.0);
                let px_per_unit = view.scale() * mm_per(unit);
                assert!([1, 2, 5].contains(&l.mantissa), "{unit:?} {pct}");
                let step = f64::from(l.mantissa) * 10f64.powi(l.exponent);
                assert!((l.major_px - step * px_per_unit).abs() < 1e-6 * l.major_px.max(1.0));
                assert!(l.major_px >= 40.0 - 1e-9, "{unit:?} {pct}: {}", l.major_px);
                assert!(l.major_px < 100.0 + 1e-9, "{unit:?} {pct}: {}", l.major_px);
                // minimality: the next smaller 1-2-5 step is under 40 px
                let smaller = match l.mantissa {
                    1 => 5.0 * 10f64.powi(l.exponent - 1),
                    2 => 1.0 * 10f64.powi(l.exponent),
                    _ => 2.0 * 10f64.powi(l.exponent),
                };
                assert!(
                    smaller * px_per_unit < 40.0 + 1e-9,
                    "{unit:?} {pct}: smaller step {smaller} still >= 40 px"
                );
            }
        }
    }
}

#[test]
fn ac5_step_accounts_for_widest_label_plus_4px() {
    // digit_px 8: the step must leave room for the widest *visible* label + 4.
    for unit in UNITS {
        for pct in zoom_sweep() {
            let view = view_at(pct, 0.0, 0.0);
            let l = ruler_layout(view, RulerAxis::Horizontal, 900.0, unit, 8.0, 8.0);
            let widest = l
                .majors
                .iter()
                .map(|m| exact_label(m.index * i64::from(l.mantissa), l.exponent))
                .map(|t| width_of(&t, 8.0))
                .fold(0.0, f64::max);
            // the spec's lower bound is for visible labels; majors includes
            // off-strip ones, so only assert the 40 px floor strictly and the
            // label floor for labels actually drawn.
            assert!(l.major_px >= 40.0 - 1e-9);
            let drawn_widest = l
                .labels
                .iter()
                .map(|lab| width_of(&lab.text, 8.0))
                .fold(0.0, f64::max);
            assert!(
                l.major_px >= drawn_widest + 4.0 - 1e-9,
                "{unit:?} {pct}: major {} < widest drawn {drawn_widest} + 4 (all-major widest {widest})",
                l.major_px
            );
        }
    }
}

#[test]
fn ac5_step_is_monotonic_in_zoom_without_labels() {
    for unit in UNITS {
        for axis in axes() {
            let mut prev = f64::INFINITY;
            for pct in zoom_sweep() {
                let l = ruler_layout(view_at(pct, 12.0, 7.0), axis, 900.0, unit, 0.0, 0.0);
                let step = l.step();
                assert!(
                    step <= prev * (1.0 + 1e-12),
                    "{unit:?} {pct}: {step} > {prev}"
                );
                prev = step;
            }
        }
    }
}

#[test]
fn ac5_step_is_monotonic_in_zoom_with_labels() {
    // With real label widths the widest label can change between zoom
    // steps; the step must still never grow as the maker zooms in.
    for unit in UNITS {
        let mut prev = f64::INFINITY;
        for pct in zoom_sweep() {
            let l = ruler_layout(
                view_at(pct, 500.0, 500.0),
                RulerAxis::Horizontal,
                900.0,
                unit,
                7.0,
                7.0,
            );
            let step = l.step();
            assert!(
                step <= prev * (1.0 + 1e-12),
                "{unit:?} {pct}: {step} > {prev}"
            );
            prev = step;
        }
    }
}

#[test]
fn ac5_five_minors_per_major_equally_spaced() {
    for unit in UNITS {
        for pct in [2.0, 33.0, 100.0, 777.0, 8000.0] {
            let l = ruler_layout(
                view_at(pct, 3.0, 4.0),
                RulerAxis::Horizontal,
                800.0,
                unit,
                7.0,
                7.0,
            );
            let expect_minor = l.major_px / 5.0;
            for m in &l.minors {
                // every minor lies on a multiple of major/5 from some major
                let rel = (m - l.majors[0].px) / expect_minor;
                assert!(
                    (rel - rel.round()).abs() < 1e-6,
                    "{unit:?} {pct}: minor {m}"
                );
                // and is not a major
                assert!(
                    !l.majors.iter().any(|mj| (mj.px - m).abs() < 1e-6),
                    "minor coincides with major"
                );
            }
            // four minors between two majors, so total = 4 per interval
            let intervals = l.majors.len() - 1;
            let in_range = l
                .minors
                .iter()
                .filter(|&&m| m > l.majors[0].px && m < l.majors[l.majors.len() - 1].px)
                .count();
            assert_eq!(in_range, 4 * intervals, "{unit:?} {pct}");
        }
    }
}

// ---- AC 2 / 3 / 4: position follows the view, origin, negatives ----

#[test]
fn ac2_every_major_matches_canvas_projection() {
    // 20 scripted pan/zoom steps, as criterion 2 words it.
    let mut vp = Viewport::with_document_inset();
    vp.resize(1000.0, 700.0);
    for step in 0..20 {
        match step % 4 {
            0 => vp.pan_by_screen_delta(37.0 + f64::from(step), -23.0),
            1 => vp.zoom_about(400.0 + f64::from(step), 300.0, 1.37),
            2 => vp.zoom_about(10.0, 650.0, 0.61),
            _ => vp.pan_by_screen_delta(-512.5, 311.25),
        }
        for unit in UNITS {
            for axis in axes() {
                let view = vp.view();
                let l = ruler_layout(view, axis, 900.0, unit, 7.0, 7.0);
                for m in &l.majors {
                    let value_mm = m.index as f64 * l.step() * mm_per(unit);
                    let (sx, sy) = view.document_to_screen(Point::new(value_mm, value_mm));
                    let want = if axis == RulerAxis::Horizontal {
                        sx
                    } else {
                        sy
                    };
                    assert!(
                        (m.px - want).abs() <= 0.5,
                        "step {step} {unit:?} {axis:?}: major {} at {} want {want}",
                        m.index,
                        m.px
                    );
                }
            }
        }
    }
}

#[test]
fn ac3_origin_tick_is_document_corner_and_values_grow_right_and_down() {
    let view = view_at(100.0, -50.0, -30.0);
    let h = ruler_layout(
        view,
        RulerAxis::Horizontal,
        800.0,
        DisplayUnit::Mm,
        7.0,
        7.0,
    );
    let v = ruler_layout(view, RulerAxis::Vertical, 800.0, DisplayUnit::Mm, 7.0, 7.0);
    let (cx, cy) = view.document_to_screen(Point::new(0.0, 0.0));
    assert!((h.origin_px.unwrap() - cx).abs() < 1e-9);
    assert!((v.origin_px.unwrap() - cy).abs() < 1e-9);
    // index 0 major at that px; larger index further right/down
    let h0 = h.majors.iter().find(|m| m.index == 0).unwrap();
    assert!((h0.px - cx).abs() < 1e-6);
    for w in h.majors.windows(2) {
        assert_eq!(w[1].index, w[0].index + 1, "contiguous indices");
        assert!(w[1].px > w[0].px);
        assert!((w[1].px - w[0].px - h.major_px).abs() < 1e-6);
    }
    let v0 = v.majors.iter().find(|m| m.index == 0).unwrap();
    assert!((v0.px - cy).abs() < 1e-6);
    assert!(
        v.majors
            .windows(2)
            .all(|w| w[1].px > w[0].px && w[1].index > w[0].index)
    );
}

#[test]
fn ac3_no_origin_when_zero_is_off_strip() {
    let right = ruler_layout(
        view_at(100.0, 5000.0, 0.0),
        RulerAxis::Horizontal,
        800.0,
        DisplayUnit::Mm,
        7.0,
        7.0,
    );
    assert_eq!(right.origin_px, None);
    let neg = ruler_layout(
        view_at(100.0, -5000.0, 0.0),
        RulerAxis::Horizontal,
        800.0,
        DisplayUnit::Mm,
        7.0,
        7.0,
    );
    assert_eq!(neg.origin_px, None);
}

#[test]
fn ac4_rulers_continue_with_negative_values() {
    let view = view_at(100.0, -200.0, -200.0);
    for axis in axes() {
        let l = ruler_layout(view, axis, 800.0, DisplayUnit::Mm, 7.0, 7.0);
        assert!(l.labels.iter().any(|x| x.text.starts_with(MINUS)));
        assert!(l.labels.iter().any(|x| x.text == "0"));
        assert!(
            l.labels.iter().all(|x| !x.text.starts_with('-')),
            "ASCII hyphen used"
        );
        assert!(l.labels.iter().all(|x| x.text != format!("{MINUS}0")));
    }
    // beyond the A4 size too
    let far = ruler_layout(
        view_at(100.0, 400.0, 0.0),
        RulerAxis::Horizontal,
        800.0,
        DisplayUnit::Mm,
        7.0,
        7.0,
    );
    assert!(
        far.labels
            .iter()
            .all(|x| x.text.chars().next().unwrap().is_ascii_digit())
    );
    assert_ne!(far.labels.len(), 0);
}

// ---- AC 6: exact labels ----

#[test]
fn ac6_labels_are_exact_decimals_everywhere() {
    for unit in UNITS {
        for pct in zoom_sweep() {
            for (ox, oy) in [
                (0.0, 0.0),
                (-123.4, -7.7),
                (999_990.0, 1_000_000.0),
                (-1_000_000.0, 3.3),
            ] {
                for axis in axes() {
                    let view = view_at(pct, ox, oy);
                    let l = ruler_layout(view, axis, 900.0, unit, 7.0, 7.0);
                    for lab in &l.labels {
                        let idx = label_for_major(&l, lab.tick_px);
                        let want = exact_label(idx * i64::from(l.mantissa), l.exponent);
                        assert_eq!(
                            lab.text, want,
                            "{unit:?} {pct}% origin ({ox},{oy}) {axis:?}"
                        );
                        assert!(!lab.text.contains('e') && !lab.text.contains('E'));
                        assert!(!lab.text.contains("NaN") && !lab.text.contains("inf"));
                        if lab.text.contains('.') {
                            assert!(!lab.text.ends_with('0'), "trailing zero {}", lab.text);
                            assert!(!lab.text.ends_with('.'));
                        }
                        assert!(!lab.text.contains(' ') && !lab.text.contains(','));
                        assert!(
                            !lab.text.contains("mm")
                                && !lab.text.contains("cm")
                                && !lab.text.contains("in")
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ac6_tick_at_0_3_reads_0_3() {
    // A step of 0.1 exists in inches (about 420 % to 8000 %); the tick at
    // 0.3 must read "0.3" and no label may carry float noise anywhere.
    let mut saw = false;
    for unit in UNITS {
        for pct in zoom_sweep() {
            let l = ruler_layout(
                view_at(pct, -5.0, 0.0),
                RulerAxis::Horizontal,
                900.0,
                unit,
                0.0,
                0.0,
            );
            for lab in &l.labels {
                assert_ne!(lab.text, "0.30000000000000004");
                assert!(lab.text.len() <= 12, "{}", lab.text);
                saw |= lab.text == "0.3";
            }
        }
    }
    assert!(saw, "no zoom produced the 0.3 label");
}

#[test]
fn ac6_no_negative_zero_label() {
    for pct in zoom_sweep() {
        let l = ruler_layout(
            view_at(pct, 0.0, 0.0),
            RulerAxis::Horizontal,
            900.0,
            DisplayUnit::Mm,
            7.0,
            7.0,
        );
        for lab in &l.labels {
            assert_ne!(lab.text, format!("{MINUS}0"));
            assert_ne!(lab.text, "-0");
            assert_ne!(lab.text, "0.0");
        }
        let neg_zero = ViewTransform::new(l.major_px, Point::new(-0.0, -0.0));
        let _ = neg_zero;
    }
}

// ---- AC 7: no clipping, no overlap, thinning ----

#[test]
fn ac7_labels_inside_strip_and_never_overlap() {
    for unit in UNITS {
        for pct in zoom_sweep() {
            for (ox, oy) in [
                (0.0, 0.0),
                (-1_000_000.0, -1_000_000.0),
                (999_999.8, 999_999.8),
                (-70.0, 12.5),
            ] {
                for axis in axes() {
                    for (len, digit) in [(900.0, 7.0), (24.0, 7.0), (60.0, 8.0), (300.0, 7.0)] {
                        let l = ruler_layout(view_at(pct, ox, oy), axis, len, unit, digit, digit);
                        let mut prev_end = f64::NEG_INFINITY;
                        for lab in &l.labels {
                            let start = lab.tick_px + 4.0;
                            let end = start + width_of(&lab.text, digit);
                            assert!(
                                start >= -1e-9,
                                "label starts left of strip: {} {start}",
                                lab.text
                            );
                            assert!(
                                end <= len + 1e-9,
                                "label {} ends {end} past strip {len}",
                                lab.text
                            );
                            assert!(
                                start >= prev_end - 1e-9,
                                "label {} overlaps previous",
                                lab.text
                            );
                            prev_end = end;
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ac7_thinning_uses_every_second_or_fifth_and_keeps_all_ticks() {
    // Wide labels near +-1,000,000 at low zoom force thinning.
    let mut saw_thin = false;
    for unit in UNITS {
        for pct in zoom_sweep() {
            let l = ruler_layout(
                view_at(pct, 999_000.0, 999_000.0),
                RulerAxis::Horizontal,
                900.0,
                unit,
                8.0,
                8.0,
            );
            assert!([1, 2, 5].contains(&l.label_every));
            if l.label_every > 1 {
                saw_thin = true;
                // ticks are never removed
                assert!(l.majors.len() >= 2);
                assert!(l.majors.windows(2).all(|w| w[1].index == w[0].index + 1));
            }
            // labelled ticks are on multiples of label_every
            for lab in &l.labels {
                let idx = label_for_major(&l, lab.tick_px);
                assert_eq!(
                    idx.rem_euclid(i64::from(l.label_every)),
                    0,
                    "{unit:?} {pct}"
                );
            }
        }
    }
    assert!(saw_thin, "never thinned with 8 px digits near 1,000,000");
}

#[test]
fn ac7_thinning_when_label_plus_8px_exceeds_major() {
    // Where the major spacing is less than widest label + 8, labels must
    // thin; where it is at least that, they must not (all labelled).
    for unit in UNITS {
        for pct in zoom_sweep() {
            let l = ruler_layout(
                view_at(pct, 100.0, 100.0),
                RulerAxis::Horizontal,
                1200.0,
                unit,
                8.0,
                8.0,
            );
            if l.labels.len() < 2 {
                continue;
            }
            let widest = l
                .labels
                .iter()
                .map(|x| width_of(&x.text, 8.0))
                .fold(0.0, f64::max);
            let spacing = l.major_px * f64::from(l.label_every);
            assert!(
                spacing >= widest + 0.0 - 1e-9,
                "labels touch at {unit:?} {pct}"
            );
            assert!(
                l.label_every != 1 || l.major_px >= widest + 4.0,
                "{unit:?} {pct}: labels every tick but major {} < widest {widest}+4",
                l.major_px
            );
        }
    }
}

#[test]
fn ac7_numbers_are_never_abbreviated() {
    for pct in zoom_sweep() {
        let l = ruler_layout(
            view_at(pct, 999_999.0, 0.0),
            RulerAxis::Horizontal,
            900.0,
            DisplayUnit::Mm,
            7.0,
            7.0,
        );
        for lab in &l.labels {
            assert!(
                lab.text
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == MINUS),
                "{}",
                lab.text
            );
        }
    }
}

// ---- degenerate / hostile input ----

#[test]
fn degenerate_strip_lengths_and_digit_widths_do_not_panic_or_blow_up() {
    for len in [0.0, -1.0, 1e-9, 1.0, f64::NAN, f64::NEG_INFINITY, 20_000.0] {
        for digit in [0.0, -3.0, f64::NAN, f64::INFINITY, 7.0, 1e9] {
            for unit in UNITS {
                for axis in axes() {
                    let l = ruler_layout(view_at(100.0, 0.0, 0.0), axis, len, unit, digit, digit);
                    assert!(
                        l.majors.len() < 100_000,
                        "len {len} digit {digit}: {} majors",
                        l.majors.len()
                    );
                    assert!(l.labels.len() < 100_000);
                    assert!(l.minors.len() < 500_000);
                    assert!(l.majors.iter().all(|m| m.px.is_finite()));
                    assert!(l.minors.iter().all(|m| m.is_finite()));
                    assert!(
                        l.labels
                            .iter()
                            .all(|x| x.tick_px.is_finite() && !x.text.is_empty())
                    );
                    assert!(l.major_px.is_finite());
                }
            }
        }
    }
}

#[test]
fn extreme_finite_view_values_do_not_panic_or_blow_up() {
    // Scales inside the zoom range (2 % to 8000 %), origins far past the
    // +-1,000,000 of criterion 7 and past i64: no panic, bounded output,
    // finite positions, labels only digits, point and minus sign.
    let scales = [
        0.02 * PX_PER_MM_AT_100,
        PX_PER_MM_AT_100,
        80.0 * PX_PER_MM_AT_100,
    ];
    let origins = [
        1e300, -1e300, 1e19, -1e19, 9.3e18, 1e18, -1e18, 1e15, 1e12, 0.0,
    ];
    for s in scales {
        for ox in origins {
            for unit in UNITS {
                for axis in axes() {
                    let view = ViewTransform::new(s, Point::new(ox, ox));
                    let l = ruler_layout(view, axis, 900.0, unit, 7.0, 7.0);
                    assert!(
                        l.majors.len() < 100_000,
                        "scale {s} origin {ox}: {}",
                        l.majors.len()
                    );
                    assert!(l.minors.len() < 500_000);
                    assert!(
                        l.majors.iter().all(|m| m.px.is_finite()),
                        "scale {s} origin {ox}"
                    );
                    assert!(l.minors.iter().all(|m| m.is_finite()));
                    assert!(l.labels.iter().all(|x| x.tick_px.is_finite()));
                    for lab in &l.labels {
                        assert!(
                            lab.text
                                .chars()
                                .all(|c| c.is_ascii_digit() || c == '.' || c == MINUS),
                            "scale {s} origin {ox}: {:?}",
                            lab.text
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unusable_scales_give_an_empty_layout() {
    for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        for unit in UNITS {
            let l = ruler_layout(
                ViewTransform::new(s, Point::new(3.0, 3.0)),
                RulerAxis::Horizontal,
                900.0,
                unit,
                7.0,
                7.0,
            );
            assert!(
                l.majors.is_empty() && l.minors.is_empty() && l.labels.is_empty(),
                "scale {s}"
            );
            assert_eq!(l.origin_px, None);
        }
    }
}

/// DEFECT (low): a NaN or infinite view origin yields a layout with one
/// major tick whose position is NaN; the layout of an unusable scale is
/// empty, and this should be too. Remove the `ignore` once fixed.
#[test]
fn nonfinite_origin_gives_no_nonfinite_positions() {
    for ox in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for s in [
            0.02 * PX_PER_MM_AT_100,
            PX_PER_MM_AT_100,
            80.0 * PX_PER_MM_AT_100,
        ] {
            for axis in axes() {
                let l = ruler_layout(
                    ViewTransform::new(s, Point::new(ox, ox)),
                    axis,
                    900.0,
                    DisplayUnit::Mm,
                    7.0,
                    7.0,
                );
                assert!(
                    l.majors.iter().all(|m| m.px.is_finite()),
                    "scale {s} origin {ox}"
                );
                assert!(l.minors.iter().all(|m| m.is_finite()));
                assert!(l.labels.iter().all(|x| x.tick_px.is_finite()));
            }
        }
    }
}

#[test]
fn huge_pan_offsets_keep_labels_exact_where_representable() {
    // Pan 1e9 mm away at 8000 %: positions are ~3e11 px, f64 still resolves
    // 0.2 mm steps (index ~ 5e9). The labels must still be exact decimals.
    for ox in [1.0e9, -1.0e9, 123_456_789.0] {
        let view = view_at(8000.0, ox, ox);
        let l = ruler_layout(
            view,
            RulerAxis::Horizontal,
            900.0,
            DisplayUnit::Mm,
            7.0,
            7.0,
        );
        for m in &l.majors {
            assert!(
                m.px > -200.0 && m.px < 1100.0,
                "major off the strip: {}",
                m.px
            );
        }
        for lab in &l.labels {
            let idx = label_for_major(&l, lab.tick_px);
            assert_eq!(
                lab.text,
                exact_label(idx * i64::from(l.mantissa), l.exponent)
            );
        }
    }
}

#[test]
fn majors_cover_the_whole_strip() {
    for unit in UNITS {
        for pct in zoom_sweep() {
            for axis in axes() {
                let l = ruler_layout(view_at(pct, -17.0, 4.0), axis, 700.0, unit, 7.0, 7.0);
                assert!(
                    l.majors.first().unwrap().px <= 0.0,
                    "{unit:?} {pct} first {}",
                    l.majors[0].px
                );
                assert!(l.majors.last().unwrap().px >= 700.0, "{unit:?} {pct}");
                assert!(l.majors.first().unwrap().px > -l.major_px - 1e-6);
                assert!(l.majors.last().unwrap().px < 700.0 + l.major_px + 1e-6);
            }
        }
    }
}

// ---- AC 11a: initial view ----

#[test]
fn ac11a_initial_view_puts_the_corner_128px_in() {
    let vp = Viewport::with_document_inset();
    let (x, y) = vp.view().document_to_screen(Point::new(0.0, 0.0));
    assert!(
        (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9,
        "({x}, {y})"
    );
    assert_eq!(vp.zoom_percent(), 100);
    assert!((DOCUMENT_INSET_PX - 128.0).abs() < 1e-12);
    let d = vp.screen_to_document(128.0, 128.0);
    assert!(d.x.abs() < 1e-9 && d.y.abs() < 1e-9);
}

fn corner(vp: &Viewport) -> (f64, f64) {
    vp.view().document_to_screen(Point::new(0.0, 0.0))
}

#[test]
fn ac11a_survives_first_size_reports_and_resizes_until_first_pan() {
    let mut vp = Viewport::with_document_inset();
    // host reports: first size, then several different ones
    for (w, h) in [
        (1200.0, 800.0),
        (1100.0, 700.0),
        (500.0, 400.0),
        (1920.0, 1080.0),
        (800.0, 600.0),
    ] {
        vp.resize(w, h);
        let (x, y) = corner(&vp);
        assert!(
            (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9,
            "after {w}x{h}: ({x}, {y})"
        );
    }
    // zero-sized reports (hidden pane) must not shift it either
    vp.resize(0.0, 0.0);
    vp.resize(900.0, 650.0);
    let (x, y) = corner(&vp);
    assert!(
        (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9,
        "after zero report: ({x}, {y})"
    );
}

#[test]
fn ac11a_panel_toggle_keeps_the_corner_too() {
    let mut vp = Viewport::with_document_inset();
    vp.resize(1000.0, 700.0);
    vp.keep_origin_for_width_change(-280.0);
    vp.resize(720.0, 700.0);
    let (x, y) = corner(&vp);
    assert!(
        (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9,
        "({x}, {y})"
    );
}

#[test]
fn ac11a_first_pan_ends_the_inset_and_resizes_keep_the_centre() {
    let mut vp = Viewport::with_document_inset();
    vp.resize(1000.0, 700.0);
    vp.pan_by_screen_delta(10.0, 0.0);
    let (x0, y0) = corner(&vp);
    let centre_doc = vp.screen_to_document(500.0, 350.0);
    vp.resize(600.0, 500.0);
    let after = vp.screen_to_document(300.0, 250.0);
    assert!((after.x - centre_doc.x).abs() < 1e-9 && (after.y - centre_doc.y).abs() < 1e-9);
    let (x1, _) = corner(&vp);
    assert!(
        (x1 - x0).abs() > 1.0,
        "corner did not move on a post-pan resize ({x0} -> {x1})"
    );
    let _ = y0;
}

#[test]
fn ac11a_first_zoom_and_drag_pan_end_the_inset() {
    let mut zoomed = Viewport::with_document_inset();
    zoomed.resize(1000.0, 700.0);
    zoomed.zoom_about(500.0, 350.0, 2.0);
    let c = zoomed.screen_to_document(500.0, 350.0);
    zoomed.resize(600.0, 500.0);
    let a = zoomed.screen_to_document(300.0, 250.0);
    assert!((a.x - c.x).abs() < 1e-9 && (a.y - c.y).abs() < 1e-9);

    let mut dragged = Viewport::with_document_inset();
    dragged.resize(1000.0, 700.0);
    dragged.begin_drag_pan(100.0, 100.0);
    dragged.continue_drag_pan(150.0, 120.0);
    dragged.end_drag_pan();
    let c = dragged.screen_to_document(500.0, 350.0);
    dragged.resize(600.0, 500.0);
    let a = dragged.screen_to_document(300.0, 250.0);
    assert!((a.x - c.x).abs() < 1e-9 && (a.y - c.y).abs() < 1e-9);
}

#[test]
fn ac11a_zoom_stays_cursor_fixed_from_inset_view() {
    let mut vp = Viewport::with_document_inset();
    vp.resize(1000.0, 700.0);
    vp.zoom_about(128.0, 128.0, 3.0);
    let (x, y) = corner(&vp);
    assert!((x - 128.0).abs() < 1e-6 && (y - 128.0).abs() < 1e-6);
    // clamped at the limits
    for _ in 0..30 {
        vp.zoom_about(128.0, 128.0, 10.0);
    }
    assert_eq!(vp.zoom_percent(), 8000);
    for _ in 0..60 {
        vp.zoom_about(128.0, 128.0, 0.1);
    }
    assert_eq!(vp.zoom_percent(), 2);
}

#[test]
fn plain_new_viewport_is_unchanged_by_the_inset_feature() {
    let mut vp = Viewport::new();
    let (x, y) = corner(&vp);
    assert!(x.abs() < 1e-12 && y.abs() < 1e-12);
    vp.resize(1000.0, 700.0);
    vp.resize(600.0, 500.0);
    let c = vp.screen_to_document(300.0, 250.0);
    assert!(c.x.is_finite() && c.y.is_finite());
}

/// Widest label among the majors of the layout (up to one step outside the
/// strip: the layout cannot know a label beyond the edge is not drawn), px.
fn widest_in_strip(l: &RulerLayout, len: f64, digit: f64) -> f64 {
    l.majors
        .iter()
        .filter(|m| m.px >= -l.major_px && m.px <= len + l.major_px)
        .map(|m| {
            width_of(
                &exact_label(m.index * i64::from(l.mantissa), l.exponent),
                digit,
            )
        })
        .fold(0.0, f64::max)
}

#[test]
fn ac7_labels_on_every_tick_only_with_8px_room_and_thinned_only_when_needed() {
    let len = 900.0;
    let digit = 8.0;
    for unit in UNITS {
        for pct in zoom_sweep() {
            for (ox, oy) in [(0.0, 0.0), (-999_000.0, 0.0), (123_456.7, 0.0)] {
                let l = ruler_layout(
                    view_at(pct, ox, oy),
                    RulerAxis::Horizontal,
                    len,
                    unit,
                    digit,
                    digit,
                );
                let widest = widest_in_strip(&l, len, digit);
                let drawn = l
                    .labels
                    .iter()
                    .map(|x| width_of(&x.text, digit))
                    .fold(0.0, f64::max);
                if l.label_every == 1 {
                    assert!(
                        l.major_px >= drawn + 8.0 - 1e-9,
                        "{unit:?} {pct} ({ox}): labels on every tick with only {} px for {drawn} px labels",
                        l.major_px
                    );
                } else {
                    // thinned: one step less thinning would not have fit
                    let less = if l.label_every == 5 { 2.0 } else { 1.0 };
                    assert!(
                        l.major_px * less < widest + 8.0 + 1e-9,
                        "{unit:?} {pct} ({ox}): thinned to every {} although {} px holds {widest} px labels",
                        l.label_every,
                        l.major_px * less
                    );
                }
            }
        }
    }
}
