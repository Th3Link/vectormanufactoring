//! Independent tester cases for `0045-document-formats-library` at the
//! `Session` level: the quick selection and the subject line, a pick as the
//! same resize as typing a size, the list view, the add / edit / delete flows
//! with their messages, the broken user file, import and export notices, and
//! the project file knowing nothing about it. Written from `specification.md`
//! before the implementation was read (only the public names were listed to
//! compile against).
//!
//! Criteria: 8, 9, 10, 11, 13 to 16, 18 to 22, 24, 25 (texts), 26 to 28.

#![allow(
    unused_must_use,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::similar_names,
    missing_docs,
    clippy::assert_is_empty,
    clippy::collapsible_if,
    clippy::precedence,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, DisplayUnit, Document, ObjectSnapshot, Orientation, Point, Shape,
    unpack,
};
use curvyo_editor_wasm::{DocumentSide, FormTexts, FormatEdit, Session, SizeOutcome, Tool};
use curvyo_ui_core::{FormField, FormatListView};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn size(s: &Session) -> (f64, f64) {
    let z = doc_of(s).size();
    (z.width.as_mm(), z.height.as_mm())
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn changes(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn with_rect() -> Session {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(10.0, 10.0), false);
    s.pointer_up(pt(60.0, 60.0), false, false);
    s.set_tool(Tool::Select);
    s.escape();
    s
}

fn rect_origin(s: &Session) -> (f64, f64) {
    let d = doc_of(s);
    for id in d.object_ids() {
        if let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() {
            if let Shape::Rect { bounds, .. } = p.shape {
                return (bounds.origin.x, bounds.origin.y);
            }
        }
    }
    panic!("no rectangle");
}

/// The `format_version` the container's manifest declares.
fn container_format_version(bytes: &[u8]) -> u64 {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut text = String::new();
        if f.read_to_string(&mut text).is_err() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(n) = v.get("format_version").and_then(serde_json::Value::as_u64) {
                return n;
            }
        }
    }
    panic!("no format_version in the container");
}

fn form(name: &str, w: &str, h: &str, unit: &str, group_id: &str) -> FormTexts {
    FormTexts {
        name: name.to_string(),
        width: w.to_string(),
        height: h.to_string(),
        unit: unit.to_string(),
        group_id: group_id.to_string(),
        new_group_name: String::new(),
        orientation: "portrait".to_string(),
        favourite: true,
    }
}

fn form_new_group(name: &str, w: &str, h: &str, group: &str, orientation: &str) -> FormTexts {
    FormTexts {
        new_group_name: group.to_string(),
        orientation: orientation.to_string(),
        ..form(name, w, h, "mm", "")
    }
}

fn list(s: &Session) -> FormatListView {
    s.format_list()
}

fn group_names(s: &Session) -> Vec<String> {
    s.document_presets_view()
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect()
}

fn entries(s: &Session, group: &str) -> Vec<(String, bool)> {
    s.document_presets_view()
        .groups
        .iter()
        .find(|g| g.name == group)
        .map(|g| {
            g.entries
                .iter()
                .map(|e| (e.name.clone(), e.pressed))
                .collect()
        })
        .unwrap_or_default()
}

fn row<'a>(v: &'a FormatListView, id: &str) -> &'a curvyo_ui_core::FormatRowView {
    v.groups
        .iter()
        .flat_map(|g| g.rows.iter())
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn group<'a>(v: &'a FormatListView, id: &str) -> &'a curvyo_ui_core::FormatGroupView {
    v.groups.iter().find(|g| g.id == id).unwrap()
}

// ---------------------------------------------------------------- Part B: quick selection

#[test]
fn c8_by_default_the_section_shows_exactly_the_paper_strip() {
    let s = Session::new(1);
    let v = s.document_presets_view();
    assert_eq!(group_names(&s), ["Paper"], "no Slides strip");
    let names: Vec<String> = v.groups[0].entries.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["A0", "A1", "A2", "A3", "A4", "A5", "A6"]);
    assert_eq!(v.subject, "A4, portrait");
    assert_eq!(v.orientation, Some(Orientation::Portrait));
    let pressed: Vec<&str> = v.groups[0]
        .entries
        .iter()
        .filter(|e| e.pressed)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(pressed, ["A4"]);
    assert_eq!(v.groups[0].entries[4].accessible_name, "Paper A4");
}

#[test]
fn c11_slides_off_means_custom_and_on_means_named() {
    let mut s = Session::new(1);
    s.set_document_side(DocumentSide::Width, "508");
    s.set_document_side(DocumentSide::Height, "285.75");
    assert_eq!(s.document_presets_view().subject, "Custom");
    assert!(s.set_format_group_enabled("slides", true).ok);
    let subject = s.document_presets_view().subject;
    assert!(subject.starts_with("16:9"), "{subject}");
    assert!(s.set_format_group_enabled("slides", false).ok);
    assert_eq!(s.document_presets_view().subject, "Custom");
}

#[test]
fn c11_a_format_that_is_not_a_favourite_still_names_the_document_but_shows_no_pressed_cell() {
    let mut s = Session::new(1);
    assert!(s.set_format_favourite("a3", false).ok);
    s.set_document_side(DocumentSide::Width, "297");
    s.set_document_side(DocumentSide::Height, "420");
    let v = s.document_presets_view();
    assert_eq!(v.subject, "A3, portrait");
    assert!(v.groups[0].entries.iter().all(|e| !e.pressed));
    assert!(v.groups[0].entries.iter().all(|e| e.name != "A3"));
    // The list still marks the row as the current state.
    assert!(row(&list(&s), "a3").selected);
    // Orientation follows the document.
    assert_eq!(v.orientation, Some(Orientation::Portrait));
}

#[test]
fn c9_a_group_without_favourites_or_turned_off_draws_nothing_and_none_at_all_leaves_no_strip() {
    let mut s = Session::new(1);
    s.set_format_group_enabled("slides", true);
    assert_eq!(group_names(&s), ["Paper", "Slides"]);
    for id in ["slide-16-9", "slide-16-10", "slide-4-3"] {
        assert!(s.set_format_favourite(id, false).ok);
    }
    assert_eq!(group_names(&s), ["Paper"], "no favourite, no heading");
    for id in ["a0", "a1", "a2", "a3", "a4", "a5", "a6"] {
        assert!(s.set_format_favourite(id, false).ok);
    }
    assert!(group_names(&s).is_empty(), "no favourite at all: no strip");
    // The orientation group and the list still work.
    assert!(s.document_presets_view().orientation.is_some());
    assert_eq!(row(&list(&s), "a4").name, "A4");
    // A group switched off draws nothing even with favourites.
    s.set_format_favourite("a4", true);
    assert_eq!(group_names(&s), ["Paper"]);
    s.set_format_group_enabled("paper", false);
    assert!(group_names(&s).is_empty());
}

// ---------------------------------------------------------------- picks (criteria 10, 14)

#[test]
fn c10_a_pick_resizes_around_the_centre_in_one_commit() {
    let mut s = with_rect();
    assert!(close(rect_origin(&s), (10.0, 10.0)));
    let before = changes(&s);
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Committed);
    assert!(close(size(&s), (297.0, 420.0)));
    // Centre fixed: every object moves by half of the growth.
    assert!(close(
        rect_origin(&s),
        (10.0 + 87.0 / 2.0, 10.0 + 123.0 / 2.0)
    ));
    assert_eq!(changes(&s), before + 1, "one commit");
    // The same size again writes nothing.
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Unchanged);
    assert_eq!(changes(&s), before + 1);
    // An unknown id does nothing.
    assert_eq!(s.apply_document_preset("nope"), SizeOutcome::Invalid);
    assert_eq!(changes(&s), before + 1);
}

#[test]
fn c10_a_pick_and_the_typed_size_end_in_the_same_state() {
    // Same objects, same centre rule: pick A3, or type 297 x 420.
    let mut a = with_rect();
    let mut b = with_rect();
    a.apply_document_preset("a3");
    b.set_document_side(DocumentSide::Width, "297");
    b.set_document_side(DocumentSide::Height, "420");
    assert!(close(size(&a), size(&b)));
    assert!(close(rect_origin(&a), rect_origin(&b)));
    assert_eq!(
        a.document_presets_view().subject,
        b.document_presets_view().subject
    );
}

#[test]
fn c10_c14_a_pick_of_a_user_format_follows_the_group_orientation_rule() {
    let mut s = Session::new(1);
    let e = s.add_format(&form_new_group(
        "Key ring",
        "30",
        "50",
        "Laser",
        "landscape",
    ));
    assert!(e.ok, "{e:?}");
    let id = list(&s)
        .groups
        .iter()
        .find(|g| g.name == "Laser")
        .unwrap()
        .rows[0]
        .id
        .clone();
    assert_eq!(s.apply_document_preset(&id), SizeOutcome::Committed);
    assert!(
        close(size(&s), (50.0, 30.0)),
        "Laser opens landscape: {:?}",
        size(&s)
    );
    let subject = s.document_presets_view().subject;
    assert!(subject.starts_with("Key ring"), "{subject}");
    // The quick selection has the Laser strip with the format.
    assert_eq!(entries(&s, "Laser"), [("Key ring".to_string(), true)]);
    // A portrait group's format: back to portrait.
    let e = s.add_format(&form("Tag", "20", "70", "mm", "paper"));
    assert!(e.ok, "{e:?}");
    s.apply_document_preset("u-tag");
    assert!(close(size(&s), (20.0, 70.0)), "{:?}", size(&s));
}

#[test]
fn c14_pressing_a_format_row_is_the_same_as_the_cell() {
    let mut a = with_rect();
    let mut b = with_rect();
    // The list's rows carry the same ids as the cells.
    let id = row(&list(&a), "a5").id.clone();
    a.apply_document_preset(&id);
    b.apply_document_preset("a5");
    assert!(close(size(&a), size(&b)));
    assert!(row(&list(&a), "a5").selected);
    assert!(!row(&list(&a), "a4").selected);
}

// ---------------------------------------------------------------- Part C: the list view

#[test]
fn c13_the_list_view_names_counts_and_flags() {
    let s = Session::new(1);
    let v = list(&s);
    assert_eq!(v.total_text, "7 formats", "formats of groups that are on");
    let ids: Vec<&str> = v.groups.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, ["paper", "slides"]);
    let paper = group(&v, "paper");
    assert_eq!(paper.count_text, "7");
    assert!(paper.enabled && paper.can_toggle && !paper.can_edit);
    assert_eq!(paper.show_name, "Show group Paper");
    assert!(paper.fold_name.contains("Paper"));
    let slides = group(&v, "slides");
    assert_eq!(slides.count_text, "Off");
    assert!(!slides.enabled);
    assert!(
        slides.rows.is_empty(),
        "a group that is off shows its header only"
    );
    let a4 = row(&v, "a4");
    assert_eq!(a4.apply_name, "Apply A4, 210 \u{d7} 297 mm");
    assert_eq!(a4.size_text, "210 \u{d7} 297 mm");
    assert_eq!(a4.star_name, "Quick selection: A4");
    assert!(a4.favourite && a4.can_star && a4.selected);
    assert!(!a4.can_edit, "built-in rows have no edit or delete");
    assert!(v.can_add);
    assert!(!v.can_export, "no export without a user format");
}

#[test]
fn c13_star_name_is_constant_and_the_state_is_in_the_flag() {
    let mut s = Session::new(1);
    let before = row(&list(&s), "a4").star_name.clone();
    s.set_format_favourite("a4", false);
    let r = row(&list(&s), "a4").clone();
    assert_eq!(r.star_name, before);
    assert!(!r.favourite);
}

#[test]
fn c13_total_counts_only_groups_that_are_on() {
    let mut s = Session::new(1);
    s.set_format_group_enabled("slides", true);
    assert_eq!(list(&s).total_text, "10 formats");
    assert_eq!(group(&list(&s), "slides").count_text, "3");
    s.set_format_group_enabled("slides", false);
    s.set_format_group_enabled("paper", false);
    assert_eq!(list(&s).total_text, "0 formats");
}

#[test]
fn c15_a_star_changes_the_quick_selection_in_the_same_call_and_returns_the_file_text() {
    let mut s = Session::new(1);
    let e = s.set_format_favourite("a4", false);
    assert!(e.ok);
    assert!(!e.file_text.is_empty(), "the host writes this text");
    let names: Vec<String> = entries(&s, "Paper").into_iter().map(|e| e.0).collect();
    assert!(!names.contains(&"A4".to_string()));
    assert_eq!(names.len(), 6);
    // The file text reproduces the state in a new session.
    let mut t = Session::new(2);
    t.load_formats(&e.file_text).unwrap();
    assert_eq!(entries(&t, "Paper"), entries(&s, "Paper"));
    // Starring again restores it.
    s.set_format_favourite("a4", true);
    assert_eq!(entries(&s, "Paper").len(), 7);
}

#[test]
fn c16_a_group_switch_updates_quick_selection_and_subject_and_returns_the_text() {
    let mut s = Session::new(1);
    s.set_document_side(DocumentSide::Width, "508");
    s.set_document_side(DocumentSide::Height, "285.75");
    let e = s.set_format_group_enabled("slides", true);
    assert!(e.ok && !e.file_text.is_empty());
    assert_eq!(group_names(&s), ["Paper", "Slides"]);
    assert_eq!(entries(&s, "Slides").len(), 3);
    let mut t = Session::new(2);
    t.load_formats(&e.file_text).unwrap();
    assert_eq!(group_names(&t), ["Paper", "Slides"]);
    assert!(!s.set_format_group_enabled("nope", true).ok);
}

// ---------------------------------------------------------------- Part D: forms

#[test]
fn c18_the_add_form_opens_prefilled_with_the_document_size_in_the_display_unit() {
    let s = Session::new(1);
    let p = s.format_prefill("", "");
    assert_eq!(p.name, "");
    assert_eq!(
        (p.width.as_str(), p.height.as_str(), p.unit),
        ("210", "297", "mm")
    );
    assert_eq!(p.group_id, "", "no user group yet: New group");
    assert!(p.favourite);
    assert_eq!(s.display_unit(), DisplayUnit::Mm);

    let mut s = Session::new(1);
    s.add_format(&form_new_group("Ring", "30", "50", "Laser", "landscape"));
    let p = s.format_prefill("", "");
    assert_eq!(p.group_id, "u-laser", "else the first user group");
    let p = s.format_prefill("", "paper");
    assert_eq!(p.group_id, "paper", "the group last used");
    let p = s.format_prefill("", "ghost");
    assert_eq!(p.group_id, "u-laser", "an unknown last group falls back");
}

#[test]
fn c18_the_prefill_of_an_edit_is_the_formats_values_in_the_unit_typed() {
    let mut s = Session::new(1);
    assert!(s.add_format(&form("Postcard", "4", "6", "in", "paper")).ok);
    let p = s.format_prefill("u-postcard", "paper");
    assert_eq!(p.name, "Postcard");
    assert_eq!(
        (p.width.as_str(), p.height.as_str(), p.unit),
        ("4", "6", "in")
    );
    assert_eq!(p.group_id, "paper");
}

#[test]
fn c20_a_valid_form_creates_the_format_writes_text_and_leaves_the_document_alone() {
    let mut s = with_rect();
    let before_changes = changes(&s);
    let before_size = size(&s);
    let e = s.add_format(&form("Key ring", "30", "50", "mm", "paper"));
    assert!(e.ok, "{e:?}");
    assert_eq!(e.notice, "Added Key ring.");
    assert!(e.errors.is_empty());
    assert!(!e.file_text.is_empty());
    assert_eq!(changes(&s), before_changes, "the document is not changed");
    assert_eq!(size(&s), before_size);
    let r = row(&list(&s), "u-key-ring").clone();
    assert_eq!(r.size_text, "30 \u{d7} 50 mm");
    assert!(r.can_edit && r.favourite);
    assert_eq!(r.edit_name, "Edit Key ring");
    assert_eq!(r.delete_name, "Delete Key ring");
    assert_eq!(r.delete_prompt, "Delete Key ring?");
    assert!(list(&s).can_export);
    // In the quick selection because the star was on.
    assert!(entries(&s, "Paper").iter().any(|e| e.0 == "Key ring"));
    // With the star off, not in the quick selection.
    let mut f = form("Plate", "31", "51", "mm", "paper");
    f.favourite = false;
    assert!(s.add_format(&f).ok);
    assert!(!entries(&s, "Paper").iter().any(|e| e.0 == "Plate"));
    assert!(!row(&list(&s), "u-plate").favourite);
}

#[test]
fn c20_every_message_of_the_spec() {
    let mut s = Session::new(1);
    fn msgs(s: &mut Session, f: FormTexts) -> Vec<(FormField, String)> {
        let e = s.add_format(&f);
        assert!(!e.ok, "{f:?}");
        assert!(e.file_text.is_empty(), "nothing written");
        e.errors.into_iter().map(|x| (x.field, x.message)).collect()
    }
    assert_eq!(
        msgs(&mut s, form("", "30", "50", "mm", "paper"))[0],
        (
            FormField::Name,
            "Enter a name of 1 to 24 characters".to_string()
        )
    );
    assert_eq!(
        msgs(&mut s, form(&"n".repeat(25), "30", "50", "mm", "paper"))[0].1,
        "Enter a name of 1 to 24 characters"
    );
    assert_eq!(
        msgs(&mut s, form("X", "abc", "50", "mm", "paper"))[0],
        (
            FormField::Width,
            "Enter a number from 1 to 100000 mm".to_string()
        )
    );
    assert_eq!(
        msgs(&mut s, form("X", "30", "0.5", "mm", "paper"))[0],
        (
            FormField::Height,
            "Enter a number from 1 to 100000 mm".to_string()
        )
    );
    assert_eq!(
        msgs(&mut s, form("X", "30", "100001", "mm", "paper"))[0].1,
        "Enter a number from 1 to 100000 mm"
    );
    let cm = msgs(&mut s, form("X", "0", "50", "cm", "paper"));
    assert!(
        cm[0].1.starts_with("Enter a number from ") && cm[0].1.ends_with(" cm"),
        "{cm:?}"
    );
    assert!(
        cm[0].1.contains("10000"),
        "limit in the chosen unit: {cm:?}"
    );
    let inch = msgs(&mut s, form("X", "0", "50", "in", "paper"));
    assert!(
        inch[0].1.ends_with(" in") && !inch[0].1.contains("100000"),
        "{inch:?}"
    );
    assert_eq!(
        msgs(&mut s, form("X", "21", "29.7", "cm", "paper"))[0].1,
        "Same size as A4"
    );
    // All invalid fields are reported in form order.
    let all = msgs(&mut s, form("", "x", "y", "mm", "paper"));
    let fields: Vec<FormField> = all.iter().map(|x| x.0).collect();
    assert_eq!(
        fields,
        [FormField::Name, FormField::Width, FormField::Height]
    );
    // Name used in the group.
    assert!(s.add_format(&form("Ring", "30", "50", "mm", "paper")).ok);
    assert_eq!(
        msgs(&mut s, form("Ring", "31", "51", "mm", "paper"))[0],
        (FormField::Name, "This group already has Ring".to_string())
    );
    // New group: the name is required and unique.
    let e = s.add_format(&form_new_group("X", "40", "60", "", "portrait"));
    assert!(!e.ok);
    assert_eq!(e.errors[0].field, FormField::GroupName);
    assert_eq!(e.errors[0].message, "Enter a name of 1 to 24 characters");
    let e = s.add_format(&form_new_group("X", "40", "60", "PAPER", "portrait"));
    assert!(!e.ok);
    assert_eq!(e.errors[0].field, FormField::GroupName);
}

#[test]
fn c20_the_list_is_full_at_200_and_a_message_says_so() {
    let mut s = Session::new(1);
    for i in 0..200 {
        let e = s.add_format(&form(
            &format!("F{i}"),
            "10",
            &format!("{}", 100 + i),
            "mm",
            "paper",
        ));
        assert!(e.ok, "{i}: {e:?}");
    }
    let e = s.add_format(&form("Last", "10", "999", "mm", "paper"));
    assert!(!e.ok);
    assert_eq!(e.errors[0].message, "The list is full (200 formats)");
}

#[test]
fn c20_numbers_typed_the_way_makers_type_them() {
    let mut s = Session::new(1);
    // Surrounding spaces and a trailing unit are the same family of input as the
    // document size fields accept; garbage never panics and never writes.
    for (i, text) in ["  30 ", "30.5", "3e1", "+30", "030"].iter().enumerate() {
        let e = s.add_format(&form(&format!("T{i}"), text, "50", "mm", "paper"));
        // Either accepted, or refused with the number message: both are fine,
        // but a refusal must be on the Width field.
        if !e.ok {
            assert_eq!(e.errors[0].field, FormField::Width, "{text:?}");
        }
    }
    for text in [
        "",
        " ",
        "-",
        "--5",
        "1,2,3",
        "1..2",
        "NaN",
        "inf",
        "-inf",
        "0x10",
        "١٢",
        "5mm",
        "\u{0}",
        "1e999",
        "1e-999",
        "9999999999999999999999",
    ] {
        let before = s.format_list();
        let e = s.add_format(&form("G", text, "50", "mm", "paper"));
        assert!(!e.ok, "{text:?} accepted");
        assert_eq!(e.errors[0].field, FormField::Width, "{text:?}");
        assert_eq!(s.format_list(), before, "{text:?}");
    }
}

#[test]
fn c20_a_decimal_comma_as_a_german_maker_types_it() {
    let mut s = Session::new(1);
    let e = s.add_format(&form("Komma", "21,5", "30", "cm", "paper"));
    // Report-only probe: the document size field decides what is accepted; the
    // add form must at least agree with it.
    let doc_accepts = Session::new(1).set_document_side(DocumentSide::Width, "21,5");
    assert_eq!(
        e.ok,
        doc_accepts != SizeOutcome::Invalid,
        "the format form and the size field disagree about a decimal comma"
    );
}

#[test]
fn c20_unit_conversion_is_exact_in_the_document() {
    let mut s = Session::new(1);
    assert!(s.add_format(&form("Letter", "8.5", "11", "in", "paper")).ok);
    s.apply_document_preset("u-letter");
    assert!(close(size(&s), (215.9, 279.4)), "{:?}", size(&s));
    assert_eq!(s.document_presets_view().subject, "Letter, portrait");
    // Picking is not rounding: re-reading the size gives the format again.
    assert!(row(&list(&s), "u-letter").selected);
    // Typing 215.905 (inside 0.01) still selects, 215.92 does not.
    s.set_document_side(DocumentSide::Width, "215.905");
    assert!(row(&list(&s), "u-letter").selected);
    s.set_document_side(DocumentSide::Width, "215.92");
    assert!(!row(&list(&s), "u-letter").selected);
}

#[test]
fn c21_edit_keeps_id_and_star_and_never_touches_the_document() {
    let mut s = Session::new(1);
    assert!(s.add_format(&form("Ring", "30", "50", "mm", "paper")).ok);
    s.apply_document_preset("u-ring");
    assert!(close(size(&s), (30.0, 50.0)));
    let doc_changes = changes(&s);
    // Unstar, then edit: the star state is kept.
    s.set_format_favourite("u-ring", false);
    let e = s.save_format("u-ring", &form("Ring 2", "35", "55", "mm", "paper"));
    assert!(e.ok, "{e:?}");
    assert!(
        e.notice == "Saved Ring 2." || e.notice.contains("Ring 2"),
        "{}",
        e.notice
    );
    assert!(!row(&list(&s), "u-ring").favourite, "star kept");
    assert_eq!(row(&list(&s), "u-ring").name, "Ring 2");
    assert_eq!(changes(&s), doc_changes, "the document is untouched");
    assert!(
        close(size(&s), (30.0, 50.0)),
        "even though it had that size"
    );
    assert_eq!(s.document_presets_view().subject, "Custom");
    // The old size is not a duplicate of itself.
    let e = s.save_format("u-ring", &form("Ring 2", "35", "55", "mm", "paper"));
    assert!(e.ok, "{e:?}");
    // But another format's size is.
    let e = s.save_format("u-ring", &form("Ring 2", "210", "297", "mm", "paper"));
    assert!(!e.ok);
    assert_eq!(e.errors[0].message, "Same size as A4");
    assert!(
        !s.save_format("a4", &form("A4", "211", "297", "mm", "paper"))
            .ok
    );
}

#[test]
fn c22_delete_removes_the_format_and_leaves_the_document_alone() {
    let mut s = Session::new(1);
    assert!(s.add_format(&form("Ring", "30", "50", "mm", "paper")).ok);
    s.apply_document_preset("u-ring");
    let c = changes(&s);
    let e = s.delete_format("u-ring");
    assert!(e.ok && !e.file_text.is_empty(), "{e:?}");
    assert!(!list(&s).can_export);
    assert_eq!(changes(&s), c);
    assert!(close(size(&s), (30.0, 50.0)));
    assert_eq!(s.document_presets_view().subject, "Custom");
    assert!(!s.delete_format("a4").ok, "built-ins cannot be deleted");
    assert!(!s.delete_format("u-ring").ok, "already gone");
}

#[test]
fn c22_delete_group_prompt_counts_the_formats_and_removes_them() {
    let mut s = Session::new(1);
    s.add_format(&form_new_group("Ring", "30", "50", "Laser", "landscape"));
    s.add_format(&form("Plate", "31", "51", "mm", "u-laser"));
    let g = group(&list(&s), "u-laser").clone();
    assert!(g.can_edit);
    assert_eq!(g.delete_prompt, "Delete group Laser and its 2 formats?");
    assert_eq!(g.edit_name, "Edit group Laser");
    assert_eq!(g.show_name, "Show group Laser");
    assert_eq!(g.orientation, "landscape");
    let e = s.delete_format_group("u-laser");
    assert!(e.ok, "{e:?}");
    assert!(list(&s).groups.iter().all(|g| g.id != "u-laser"));
    assert!(!s.delete_format_group("paper").ok);
    assert!(!s.delete_format_group("slides").ok);
}

#[test]
fn c24_group_edit_renames_and_sets_orientation_with_validation() {
    let mut s = Session::new(1);
    s.add_format(&form_new_group("Ring", "30", "50", "Laser", "landscape"));
    s.add_format(&form_new_group(
        "Hoop",
        "130",
        "131",
        "Embroidery",
        "portrait",
    ));
    let e = s.save_format_group("u-laser", "Laser cut", "portrait");
    assert!(e.ok, "{e:?}");
    let v = list(&s);
    let g = group(&v, "u-laser");
    assert_eq!((g.name.as_str(), g.orientation), ("Laser cut", "portrait"));
    let e = s.save_format_group("u-laser", "embroidery", "portrait");
    assert!(!e.ok);
    assert_eq!(e.errors[0].field, FormField::GroupName);
    let e = s.save_format_group("u-laser", "", "portrait");
    assert!(!e.ok);
    assert_eq!(e.errors[0].message, "Enter a name of 1 to 24 characters");
    assert!(!s.save_format_group("paper", "Papier", "portrait").ok);
}

// ---------------------------------------------------------------- Part E: import / export

const SHARE: &str = r#"
format = 1
favourites = ["u-a", "u-c"]

[[group]]
id = "u-one"
name = "One"
default_orientation = "portrait"
  [[group.preset]]
  id = "u-a"
  name = "Aaa"
  short_side = 11
  long_side = 22
  unit = "mm"
  [[group.preset]]
  id = "u-b"
  name = "Bbb"
  short_side = 12
  long_side = 23
  unit = "mm"

[[group]]
id = "u-two"
name = "Two"
default_orientation = "landscape"
  [[group.preset]]
  id = "u-c"
  name = "Ccc"
  short_side = 13
  long_side = 24
  unit = "mm"
  [[group.preset]]
  id = "u-d"
  name = "Ddd"
  short_side = 210
  long_side = 297
  unit = "mm"
"#;

#[test]
fn c25_c27_import_notices_the_spec_wording() {
    let mut s = Session::new(1);
    let e = s.import_formats(SHARE, "share.toml");
    assert!(e.ok, "{e:?}");
    assert_eq!(
        e.notice,
        "Imported 3 formats in 2 groups. Skipped 1 that was already there."
    );
    assert!(!e.file_text.is_empty());
    let v = list(&s);
    assert!(v.can_export);
    assert!(row(&v, "u-a").favourite && row(&v, "u-c").favourite);
    // Importing again: nothing new.
    let e = s.import_formats(SHARE, "share.toml");
    assert!(e.ok);
    assert!(
        e.notice.starts_with("Nothing new to import."),
        "{}",
        e.notice
    );
}

#[test]
fn c25_c27_a_refused_import_says_so_and_changes_nothing() {
    let mut s = Session::new(1);
    s.add_format(&form("Ring", "30", "50", "mm", "paper"));
    let before = list(&s);
    let e = s.import_formats("format = 2\n", "future.toml");
    assert!(!e.ok);
    assert!(e.file_text.is_empty());
    assert!(
        e.notice.starts_with("Could not import future.toml: ")
            && e.notice.ends_with(". Nothing was changed."),
        "{}",
        e.notice
    );
    assert!(
        e.notice.to_lowercase().contains("newer version"),
        "{}",
        e.notice
    );
    let e = s.import_formats("this is not toml", "x.toml");
    assert!(!e.ok && e.notice.starts_with("Could not import x.toml"));
    assert_eq!(list(&s), before);
}

#[test]
fn c26_export_is_none_without_user_formats_and_round_trips() {
    let mut s = Session::new(1);
    assert_eq!(s.export_formats(), None);
    s.import_formats(SHARE, "share.toml");
    let out = s.export_formats().expect("a user format exists");
    assert!(!out.contains("\"a4\""), "nothing built-in: {out}");
    assert!(!out.contains("enabled"), "{out}");
    let mut t = Session::new(2);
    let e = t.import_formats(&out, "curvyo-formats.toml");
    assert!(e.ok, "{e:?}");
    assert_eq!(
        e.notice, "Imported 3 formats in 2 groups.",
        "same content, nothing skipped"
    );
}

// ---------------------------------------------------------------- Part A: a broken file (criterion 6)

#[test]
fn c6_a_broken_file_keeps_built_ins_refuses_every_edit_and_hides_every_write_control() {
    let mut s = Session::new(1);
    let reason = s.load_formats("format = 1\nsurprise = 3\n").unwrap_err();
    assert!(!reason.is_empty());
    assert!(
        !reason.contains("PresetError"),
        "debug output shown to the maker: {reason}"
    );
    assert!(s.formats_broken().is_some());
    // Built-in formats, read-only.
    assert_eq!(group_names(&s), ["Paper"]);
    let v = list(&s);
    assert!(!v.can_add && !v.can_export);
    for g in &v.groups {
        assert!(!g.can_toggle && !g.can_edit, "{}", g.id);
        for r in &g.rows {
            assert!(!r.can_star && !r.can_edit, "{}", r.id);
        }
    }
    // Every edit is refused in the core, too.
    assert!(!s.add_format(&form("X", "30", "50", "mm", "paper")).ok);
    assert!(
        !s.save_format("a4", &form("X", "30", "50", "mm", "paper"))
            .ok
    );
    assert!(!s.delete_format("a4").ok);
    assert!(!s.set_format_favourite("a4", false).ok);
    assert!(!s.set_format_group_enabled("slides", true).ok);
    assert!(!s.save_format_group("paper", "X", "portrait").ok);
    assert!(!s.delete_format_group("paper").ok);
    assert!(!s.import_formats(SHARE, "share.toml").ok);
    for e in [
        s.add_format(&form("X", "30", "50", "mm", "paper")),
        s.set_format_favourite("a4", false),
        s.import_formats(SHARE, "share.toml"),
    ] {
        assert!(e.file_text.is_empty(), "nothing to write while broken");
    }
    assert_eq!(s.export_formats(), None);
    // Picking a built-in format still works.
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Committed);
}

#[test]
fn c6_set_aside_restores_a_working_library_on_the_built_ins() {
    let mut s = Session::new(1);
    s.load_formats("garbage = = =").unwrap_err();
    s.formats_file_set_aside();
    assert!(s.formats_broken().is_none());
    let v = list(&s);
    assert!(v.can_add);
    assert!(s.add_format(&form("X", "30", "50", "mm", "paper")).ok);
}

#[test]
fn c6_the_host_can_mark_the_library_broken_for_a_file_it_could_not_read() {
    let mut s = Session::new(1);
    s.mark_formats_broken("permission denied");
    assert_eq!(s.formats_broken(), Some("permission denied"));
    assert!(!s.set_format_favourite("a4", false).ok);
    assert!(!list(&s).can_add);
}

#[test]
fn c6_a_file_made_by_a_newer_version_is_a_broken_file_with_a_clear_reason() {
    let mut s = Session::new(1);
    let reason = s.load_formats("format = 2\n").unwrap_err();
    assert!(reason.to_lowercase().contains("newer version"), "{reason}");
    assert!(!s.add_format(&form("X", "30", "50", "mm", "paper")).ok);
}

#[test]
fn c3_c29_a_good_file_is_loaded_before_the_first_read_and_applied_everywhere() {
    let mut s = Session::new(1);
    let loadable = SHARE.replace("short_side = 210", "short_side = 211");
    s.load_formats(&loadable).unwrap();
    assert!(s.formats_broken().is_none());
    assert_eq!(group_names(&s), ["One", "Two"]);
    // Favourites key in the file: only u-a and u-c are favourites, nothing else:
    // the Paper strip is gone.
    assert!(group_names(&s).iter().all(|n| n != "Paper"));
    assert_eq!(entries(&s, "One").len(), 1);
    assert_eq!(entries(&s, "Two").len(), 1);
    // Loading text twice is the same as once.
    let a = s.document_presets_view();
    s.load_formats(&loadable).unwrap();
    assert_eq!(s.document_presets_view(), a);
    // An empty text is a missing file: the built-in defaults... or a refusal;
    // it must not panic and must leave a working session.
    let mut t = Session::new(3);
    let _ = t.load_formats("");
    let _ = t.document_presets_view();
}

// ---------------------------------------------------------------- Part F: not in the project

#[test]
fn c28_the_project_stores_only_its_size_and_the_format_version_is_unchanged() {
    let mut s = with_rect();
    s.add_format(&form("Ring", "30", "50", "mm", "paper"));
    s.apply_document_preset("u-ring");
    let bytes = s.pack("0.1.0").unwrap();
    let d = unpack(9, &bytes).unwrap();
    assert_eq!(CURRENT_FORMAT_VERSION, 9, "the project format did not move");
    let _ = d;
    assert_eq!(
        container_format_version(&bytes),
        u64::from(CURRENT_FORMAT_VERSION)
    );
    // The bytes contain neither the format's name nor its id.
    let hay = String::from_utf8_lossy(&bytes);
    assert!(!hay.contains("u-ring") && !hay.contains("Key ring"));
    // Delete the format, reopen: same size, no error, Custom.
    s.delete_format("u-ring");
    let reopened = Session::open(2, &bytes).unwrap();
    assert!(close(size(&reopened), (30.0, 50.0)));
    assert_eq!(reopened.document_presets_view().subject, "Custom");
    // A project saved with a library of other content has the same layout: the
    // bytes of two sessions with the same drawing and size are equally long.
    let mut a = with_rect();
    let mut b = with_rect();
    b.add_format(&form("Ring", "30", "50", "mm", "paper"));
    b.set_format_group_enabled("slides", true);
    a.set_document_side(DocumentSide::Width, "100");
    b.set_document_side(DocumentSide::Width, "100");
    assert_eq!(
        doc_of(&a).export_loro_snapshot().unwrap().len(),
        doc_of(&b).export_loro_snapshot().unwrap().len()
    );
}

#[test]
fn c28_files_saved_before_still_open_and_start_on_the_document_tab() {
    // The fixtures that shipped before this slice.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../curvyo-document-core/tests/fixtures");
    for name in [
        "valid.curvyo",
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "dash_v9.curvyo",
    ] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let s = Session::open(1, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let v = s.document_presets_view();
        assert!(!v.subject.is_empty(), "{name}");
        assert_eq!(
            s.panel_content(),
            curvyo_ui_core::PanelContent::Document,
            "{name}"
        );
    }
}

// ---------------------------------------------------------------- random forms

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn scrap(rng: &mut Rng) -> String {
    const BITS: [&str; 22] = [
        "",
        "0",
        "1",
        "30",
        "50.5",
        "-4",
        "1e3",
        "abc",
        " ",
        "NaN",
        "inf",
        "100000",
        "100001",
        "0.01",
        "99999.99",
        "\u{202e}",
        "\u{0}",
        "\u{1f4a9}",
        "21",
        "29.7",
        ",",
        "5,5",
    ];
    let n = rng.below(3) + 1;
    (0..n).map(|_| BITS[rng.below(22) as usize]).collect()
}

/// Random garbage in every field of every edit: no panic, a refusal is a
/// refusal (no file text, an error on some field, library unchanged), and an
/// accepted edit leaves a library that its own text reloads into.
#[test]
fn model_random_form_input_never_panics_and_refusals_change_nothing() {
    for seed in 1..=8_u64 {
        let mut rng = Rng(seed * 2_654_435_761 | 1);
        let mut s = with_rect();
        for step in 0..120 {
            let before = (list(&s), s.document_presets_view(), size(&s));
            let units = ["mm", "cm", "in", "px", "", "furlong"];
            let ids: Vec<String> = list(&s)
                .groups
                .iter()
                .flat_map(|g| g.rows.iter().map(|r| r.id.clone()))
                .collect();
            let mut ids = ids;
            ids.push("nope".to_string());
            let gids: Vec<String> = list(&s).groups.iter().map(|g| g.id.clone()).collect();
            let f = FormTexts {
                name: scrap(&mut rng),
                width: scrap(&mut rng),
                height: scrap(&mut rng),
                unit: units[rng.below(6) as usize].to_string(),
                group_id: if rng.below(3) == 0 {
                    String::new()
                } else {
                    gids[rng.below(gids.len() as u64) as usize].clone()
                },
                new_group_name: scrap(&mut rng),
                orientation: ["portrait", "landscape", "diagonal", ""][rng.below(4) as usize]
                    .to_string(),
                favourite: rng.below(2) == 0,
            };
            let edit: FormatEdit = match rng.below(9) {
                0..=2 => s.add_format(&f),
                3 => s.save_format(&ids[rng.below(ids.len() as u64) as usize], &f),
                4 => s.delete_format(&ids[rng.below(ids.len() as u64) as usize]),
                5 => s.set_format_favourite(
                    &ids[rng.below(ids.len() as u64) as usize],
                    rng.below(2) == 0,
                ),
                6 => s.set_format_group_enabled(
                    &gids[rng.below(gids.len() as u64) as usize],
                    rng.below(2) == 0,
                ),
                7 => s.save_format_group(
                    &gids[rng.below(gids.len() as u64) as usize],
                    &scrap(&mut rng),
                    "portrait",
                ),
                _ => s.delete_format_group(&gids[rng.below(gids.len() as u64) as usize]),
            };
            let after = (list(&s), s.document_presets_view(), size(&s));
            if edit.ok {
                assert!(!edit.file_text.is_empty(), "seed {seed} step {step}");
                let mut t = Session::new(9);
                t.load_formats(&edit.file_text).unwrap_or_else(|e| {
                    panic!(
                        "seed {seed} step {step}: own text unreadable: {e}\n{}",
                        edit.file_text
                    )
                });
                let shape = |x: &Session| -> Vec<(String, Vec<String>)> {
                    x.document_presets_view()
                        .groups
                        .iter()
                        .map(|g| {
                            (
                                g.name.clone(),
                                g.entries.iter().map(|e| e.id.clone()).collect(),
                            )
                        })
                        .collect()
                };
                assert_eq!(shape(&t), shape(&s), "seed {seed} step {step}");
            } else {
                assert!(edit.file_text.is_empty(), "seed {seed} step {step}");
                assert_eq!(
                    before, after,
                    "seed {seed} step {step}: a refusal changed something"
                );
            }
            // The document was never touched by any edit.
            assert!(close(size(&s), before.2), "seed {seed} step {step}");
            // Occasionally pick something.
            if rng.below(6) == 0 && !ids.is_empty() {
                s.apply_document_preset(&ids[rng.below(ids.len() as u64) as usize]);
            }
        }
    }
}
