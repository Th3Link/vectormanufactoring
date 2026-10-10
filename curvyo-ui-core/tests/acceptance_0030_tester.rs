//! Independent tester cases for `0030-document-size-presets`, ui-core side: the
//! pure pick rule, the orientation swap and the view, driven by lists of my own
//! (nine paper presets, an empty list, a square preset, inch presets).
//! Written from `specification.md` before the implementation was read.
//!
//! Criteria: 3 (nine presets -> nine entries), 8, 10, 10a, 13, 14, 16, 17 and
//! the run-time empty-list fallback.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    missing_docs
)]

use std::fmt::Write as _;

use curvyo_document_core::{DisplayUnit, DocumentSize, Orientation, PresetList};
use curvyo_ui_core::{orientation_swap, preset_pick_size, presets_view};

fn list(text: &str) -> PresetList {
    PresetList::parse(text).unwrap()
}

fn mm(w: f64, h: f64) -> DocumentSize {
    DocumentSize::from_mm(w, h)
}

fn nine_paper() -> PresetList {
    let mut s = String::from(
        "format = 2\n[[group]]\nid = \"paper\"\nname = \"Paper\"\ndefault_orientation = \"portrait\"\n",
    );
    for i in 0..9_u32 {
        write!(
            s,
            "[[group.preset]]\nid = \"p{i}\"\nname = \"P{i}\"\nshort_side = {}\nlong_side = {}\nunit = \"mm\"\n",
            100 + i * 7,
            200 + i * 7
        )
        .unwrap();
    }
    s.push_str(
        "[[group]]\nid = \"slides\"\nname = \"Slides\"\ndefault_orientation = \"landscape\"\n[[group.preset]]\nid = \"s1\"\nname = \"S1\"\nnote = \"a note\"\nshort_side = 1080\nlong_side = 1920\nunit = \"px\"\n",
    );
    list(&s)
}

#[test]
fn a_list_of_nine_paper_presets_gives_nine_entries_in_file_order() {
    let l = nine_paper();
    let v = presets_view(&l, mm(210.0, 297.0), DisplayUnit::Mm);
    assert_eq!(v.groups.len(), 2);
    assert_eq!(v.groups[0].entries.len(), 9);
    let ids: Vec<_> = v.groups[0].entries.iter().map(|e| e.id.clone()).collect();
    assert_eq!(ids, (0..9).map(|i| format!("p{i}")).collect::<Vec<_>>());
    assert!(
        v.groups[0]
            .entries
            .iter()
            .all(|e| e.accessible_name.starts_with("Paper P"))
    );
    assert_eq!(v.subject, "Custom");
    assert!(v.groups.iter().flat_map(|g| &g.entries).all(|e| !e.pressed));
}

#[test]
fn an_empty_list_gives_an_empty_view_and_no_pick_and_never_panics() {
    let l = PresetList::default();
    let v = presets_view(&l, mm(210.0, 297.0), DisplayUnit::Mm);
    assert_eq!(v.groups.len(), 0);
    assert_eq!(v.subject, "Custom");
    assert_eq!(v.orientation, Some(Orientation::Portrait));
    assert_eq!(preset_pick_size(&l, mm(210.0, 297.0), "a4"), None);
    // The orientation still works without presets.
    assert_eq!(
        orientation_swap(mm(300.0, 400.0), Orientation::Landscape),
        Some(mm(400.0, 300.0))
    );
}

#[test]
fn pick_rule_table_with_my_own_numbers() {
    let l = nine_paper();
    // p0 = 100 x 200, p1 = 107 x 207, p2 = 114 x 214.
    let w = |s: DocumentSize| (s.width.as_mm(), s.height.as_mm());
    // Document is p0 landscape (200 x 100): picking p1 stays landscape.
    assert_eq!(
        w(preset_pick_size(&l, mm(200.0, 100.0), "p1").unwrap()),
        (207.0, 107.0)
    );
    // Document is p0 portrait: p1 stays portrait.
    assert_eq!(
        w(preset_pick_size(&l, mm(100.0, 200.0), "p1").unwrap()),
        (107.0, 207.0)
    );
    // Custom size: group default (portrait for paper, landscape for slides).
    assert_eq!(
        w(preset_pick_size(&l, mm(300.0, 400.0), "p1").unwrap()),
        (107.0, 207.0)
    );
    assert_eq!(
        w(preset_pick_size(&l, mm(400.0, 300.0), "p1").unwrap()),
        (107.0, 207.0)
    );
    // From a paper size into slides: landscape; from slides to slides: keep.
    let s1 = preset_pick_size(&l, mm(100.0, 200.0), "s1").unwrap();
    assert_eq!(w(s1), (508.0, 285.75));
    // From slides (landscape) into paper: portrait default.
    assert_eq!(w(preset_pick_size(&l, s1, "p1").unwrap()), (107.0, 207.0));
    // Unknown id.
    assert_eq!(preset_pick_size(&l, mm(100.0, 200.0), "zzz"), None);
    assert_eq!(preset_pick_size(&l, mm(100.0, 200.0), ""), None);
}

#[test]
fn a_square_size_counts_as_portrait_inside_a_group() {
    // A group containing a square preset: a document of that square size is in
    // the group; the pick keeps "portrait" for a square.
    let l = list(
        "format = 2\n[[group]]\nid = \"g\"\nname = \"G\"\ndefault_orientation = \"landscape\"\n\
         [[group.preset]]\nid = \"sq\"\nname = \"Sq\"\nshort_side = 100\nlong_side = 100\nunit = \"mm\"\n\
         [[group.preset]]\nid = \"r\"\nname = \"R\"\nshort_side = 100\nlong_side = 150\nunit = \"mm\"\n",
    );
    let got = preset_pick_size(&l, mm(100.0, 100.0), "r").unwrap();
    assert_eq!(
        (got.width.as_mm(), got.height.as_mm()),
        (100.0, 150.0),
        "portrait, not the landscape default"
    );
    // And the square is shown as the name alone.
    let v = presets_view(&l, mm(100.0, 100.0), DisplayUnit::Mm);
    assert_eq!(v.subject, "Sq");
    assert_eq!(v.orientation, None);
}

#[test]
fn the_orientation_swap_rules() {
    assert_eq!(
        orientation_swap(mm(210.0, 297.0), Orientation::Landscape),
        Some(mm(297.0, 210.0))
    );
    assert_eq!(
        orientation_swap(mm(297.0, 210.0), Orientation::Portrait),
        Some(mm(210.0, 297.0))
    );
    assert_eq!(
        orientation_swap(mm(210.0, 297.0), Orientation::Portrait),
        None
    );
    assert_eq!(
        orientation_swap(mm(297.0, 210.0), Orientation::Landscape),
        None
    );
    assert_eq!(
        orientation_swap(mm(100.0, 100.0), Orientation::Landscape),
        None
    );
    assert_eq!(
        orientation_swap(mm(100.0, 100.0), Orientation::Portrait),
        None
    );
    assert_eq!(
        orientation_swap(mm(100.0, 100.005), Orientation::Landscape),
        None
    );
    // The swap keeps the exact numbers.
    let s = orientation_swap(mm(123.456, 789.012), Orientation::Landscape).unwrap();
    assert_eq!((s.width.as_mm(), s.height.as_mm()), (789.012, 123.456));
}

#[test]
fn tooltips_for_an_inch_authored_preset_and_a_note() {
    let l = list(
        "format = 2\n[[group]]\nid = \"us\"\nname = \"US\"\ndefault_orientation = \"portrait\"\n\
         [[group.preset]]\nid = \"letter\"\nname = \"Letter\"\nnote = \"8.5 x 11\"\nshort_side = 8.5\nlong_side = 11\nunit = \"in\"\n",
    );
    let v = presets_view(&l, mm(210.0, 297.0), DisplayUnit::Mm);
    let t = &v.groups[0].entries[0].tooltip;
    assert!(t.starts_with("215.9 \u{d7} 279.4 mm"), "{t}");
    assert!(t.ends_with("\n8.5 x 11"), "{t}");
    let v = presets_view(&l, mm(210.0, 297.0), DisplayUnit::In);
    assert!(
        v.groups[0].entries[0]
            .tooltip
            .starts_with("8.5 \u{d7} 11 in"),
        "{}",
        v.groups[0].entries[0].tooltip
    );
}

#[test]
fn the_view_is_stable_for_hostile_sizes() {
    let l = nine_paper();
    for (w, h) in [
        (0.0, 0.0),
        (f64::NAN, 5.0),
        (f64::INFINITY, 1.0),
        (-5.0, 10.0),
        (1.0e12, 1.0e12),
        (1.0, 100_000.0),
    ] {
        let v = presets_view(&l, mm(w, h), DisplayUnit::Mm);
        assert_eq!(v.groups[0].entries.len(), 9);
        let _ = preset_pick_size(&l, mm(w, h), "p0");
    }
}
