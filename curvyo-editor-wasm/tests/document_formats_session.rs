//! `Session`-level checks of the document formats library
//! (`specs/0045-document-formats-library/` criteria 6, 8 to 11, 14 to 16, 20 to
//! 22, 24, 26 to 28): the quick selection, the list, the edits and the text they
//! hand to the host, a file that cannot be read, and the project file knowing
//! nothing about any of it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::Orientation;
use curvyo_editor_wasm::{DocumentSide, FormTexts, FormatEdit, Session, SizeOutcome};

fn texts(name: &str, width: &str, height: &str, group: &str) -> FormTexts {
    FormTexts {
        name: name.to_string(),
        width: width.to_string(),
        height: height.to_string(),
        unit: "mm".to_string(),
        group_id: group.to_string(),
        new_group_name: String::new(),
        orientation: "portrait".to_string(),
        favourite: true,
    }
}

fn new_group(name: &str, orientation: &str, width: &str, height: &str, label: &str) -> FormTexts {
    FormTexts {
        new_group_name: name.to_string(),
        orientation: orientation.to_string(),
        ..texts(label, width, height, "")
    }
}

fn set_size(session: &mut Session, width: &str, height: &str) {
    session.set_document_side(DocumentSide::Width, width);
    session.set_document_side(DocumentSide::Height, height);
}

fn size_mm(session: &Session) -> (f64, f64) {
    let text = session.size_text();
    let numbers: Vec<f64> = text
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter_map(|part| part.parse().ok())
        .collect();
    (numbers[0], numbers[1])
}

fn cells(session: &Session) -> Vec<(String, bool)> {
    session
        .document_presets_view()
        .groups
        .iter()
        .flat_map(|g| g.entries.iter().map(|e| (e.name.clone(), e.pressed)))
        .collect()
}

fn groups(session: &Session) -> Vec<String> {
    session
        .document_presets_view()
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect()
}

fn with_laser() -> (Session, FormatEdit) {
    let mut session = Session::new(1);
    let edit = session.add_format(&new_group("Laser", "landscape", "30", "50", "Key ring"));
    assert!(edit.ok, "{edit:?}");
    (session, edit)
}

/// Criteria 8 and 11: the defaults show the Paper strip only, and a slide size
/// is "Custom" while Slides are off.
#[test]
fn the_defaults_show_paper_and_slides_are_custom() {
    let mut session = Session::new(1);
    assert_eq!(groups(&session), ["Paper"]);
    assert_eq!(session.document_presets_view().subject, "A4, portrait");
    set_size(&mut session, "508", "285.75");
    assert_eq!(session.document_presets_view().subject, "Custom");
    assert_eq!(session.format_list().groups[1].rows.len(), 0);
}

/// Criteria 15 and 16: a Show switch and a star change the quick selection and
/// the subject line at once, and hand the host the file text.
#[test]
fn stars_and_switches_change_the_quick_selection_and_write_the_file() {
    let mut session = Session::new(1);
    let on = session.set_format_group_enabled("slides", true);
    assert!(on.ok && on.file_text.contains("slides = true"));
    assert_eq!(groups(&session), ["Paper", "Slides"]);
    set_size(&mut session, "508", "285.75");
    assert_eq!(session.document_presets_view().subject, "16:9, landscape");
    let star = session.set_format_favourite("slide-16-9", false);
    assert!(star.ok);
    assert_eq!(cells(&session).len(), 7 + 2);
    assert_eq!(
        session.document_presets_view().subject,
        "16:9, landscape",
        "still named, no cell pressed"
    );
    assert!(cells(&session).iter().all(|(_, pressed)| !pressed));
    let off = session.set_format_group_enabled("slides", false);
    assert!(off.ok);
    assert_eq!(session.document_presets_view().subject, "Custom");
    let back = session.set_format_group_enabled("slides", true);
    assert!(back.ok);
    assert_eq!(
        cells(&session).len(),
        7 + 2,
        "the same favourites as before"
    );
}

/// Criteria 18, 20, 10 and 14: add a format in a new group; it is a strip of its
/// own; a press resizes in the group's orientation; the document is not changed
/// by the add.
#[test]
fn an_added_format_is_a_strip_and_a_press_resizes_the_document() {
    let (mut session, edit) = with_laser();
    assert_eq!(edit.notice, "Added Key ring.");
    assert!(edit.file_text.contains("u-key-ring"), "{}", edit.file_text);
    assert_eq!(size_mm(&session), (210.0, 297.0), "an add never resizes");
    assert_eq!(groups(&session), ["Paper", "Laser"]);
    assert_eq!(
        session.apply_document_preset("u-key-ring"),
        SizeOutcome::Committed
    );
    assert_eq!(size_mm(&session), (50.0, 30.0), "Laser opens landscape");
    assert_eq!(
        session.document_presets_view().subject,
        "Key ring, landscape"
    );
}

/// Criterion 20: a refused add names every refused field and writes nothing.
#[test]
fn a_refused_add_changes_nothing_and_names_the_fields() {
    let (mut session, _) = with_laser();
    let before = session.format_list();
    let bad = session.add_format(&texts("", "abc", "50", "paper"));
    assert!(!bad.ok && bad.file_text.is_empty());
    let fields: Vec<_> = bad.errors.iter().map(|e| e.field.name()).collect();
    assert_eq!(fields, ["name", "width"]);
    let same = session.add_format(&texts("My A4", "210", "297", "paper"));
    assert_eq!(same.errors[0].message, "Same size as A4");
    assert_eq!(session.format_list(), before);
}

/// Criteria 21, 22 and 24: edit, delete and the group forms.
#[test]
fn edit_delete_and_rename() {
    let (mut session, _) = with_laser();
    let saved = session.save_format("u-key-ring", &texts("Key ring", "30", "55", "u-laser"));
    assert!(saved.ok, "{saved:?}");
    assert_eq!(saved.notice, "Saved Key ring.");
    assert_eq!(
        session.apply_document_preset("u-key-ring"),
        SizeOutcome::Committed
    );
    assert_eq!(size_mm(&session), (55.0, 30.0));
    let prefill = session.format_prefill("u-key-ring", "");
    assert_eq!(
        (prefill.width.as_str(), prefill.height.as_str()),
        ("55", "30")
    );
    let renamed = session.save_format_group("u-laser", "Laser cut", "portrait");
    assert!(renamed.ok);
    assert_eq!(groups(&session), ["Paper", "Laser cut"]);
    let clash = session.save_format_group("u-laser", "paper", "portrait");
    assert_eq!(clash.errors[0].field.name(), "group-name");
    // Editing never changes the document, even when it has that size.
    assert_eq!(size_mm(&session), (55.0, 30.0));
    let gone = session.delete_format("u-key-ring");
    assert!(gone.ok && !gone.file_text.contains("u-key-ring"));
    assert!(!session.delete_format("a4").ok, "a built-in format stays");
    assert!(session.delete_format_group("u-laser").ok);
    assert_eq!(groups(&session), ["Paper"]);
    assert_eq!(size_mm(&session), (55.0, 30.0), "a delete never resizes");
}

/// Criterion 28: the project knows nothing of the library.
#[test]
fn a_project_keeps_its_size_when_the_format_is_deleted() {
    let (mut session, _) = with_laser();
    assert_eq!(
        session.apply_document_preset("u-key-ring"),
        SizeOutcome::Committed
    );
    let bytes = session.pack("0.1.0").unwrap();
    let after = session.delete_format("u-key-ring");
    assert!(after.ok);
    let mut reopened = Session::open(2, &bytes).unwrap();
    reopened.load_formats(&after.file_text).unwrap();
    assert_eq!(size_mm(&reopened), (50.0, 30.0));
    assert_eq!(reopened.document_presets_view().subject, "Custom");
}

/// Criterion 6: a file that cannot be read gives the built-ins, the reason, and
/// no edit; setting it aside lifts that.
#[test]
fn a_broken_file_runs_on_the_builtins_and_refuses_every_edit() {
    let mut session = Session::new(1);
    let reason = session.load_formats("format = 2\n").unwrap_err();
    assert_eq!(reason, "made by a newer version");
    assert_eq!(session.formats_broken(), Some("made by a newer version"));
    assert_eq!(groups(&session), ["Paper"]);
    let view = session.format_list();
    assert!(!view.can_add && !view.can_export);
    assert!(view.groups.iter().all(|g| !g.can_toggle && !g.can_edit));
    assert!(view.groups[0].rows.iter().all(|r| !r.can_star));
    assert!(!session.set_format_favourite("a4", false).ok);
    assert!(!session.set_format_group_enabled("slides", true).ok);
    assert!(!session.add_format(&texts("X", "10", "20", "paper")).ok);
    assert!(!session.import_formats("format = 1\n", "x.toml").ok);
    assert_eq!(session.export_formats(), None);
    session.formats_file_set_aside();
    assert_eq!(session.formats_broken(), None);
    assert!(session.set_format_favourite("a4", false).ok);
}

/// Criteria 26 and 27: export, and an import of the same text merges nothing
/// twice; a refused file says why.
#[test]
fn export_and_import_round_trip() {
    let (session, _) = with_laser();
    let exported = session.export_formats().expect("the maker owns a format");
    assert!(exported.contains("u-key-ring") && !exported.contains("enabled"));
    assert_eq!(Session::new(1).export_formats(), None);
    let mut other = Session::new(2);
    let imported = other.import_formats(&exported, "curvyo-formats.toml");
    assert!(imported.ok);
    assert_eq!(imported.notice, "Imported 1 format in 1 group.");
    assert_eq!(groups(&other), ["Paper", "Laser"]);
    let again = other.import_formats(&exported, "curvyo-formats.toml");
    assert_eq!(
        again.notice,
        "Nothing new to import. Skipped 1 that was already there."
    );
    let bad = other.import_formats("format = 7\n", "new.toml");
    assert!(!bad.ok);
    assert_eq!(
        bad.notice,
        "Could not import new.toml: made by a newer version. Nothing was changed."
    );
}

/// Criterion 29, 33: the host hands the text back and the library is the same.
#[test]
fn the_text_the_host_stores_loads_into_a_new_session() {
    let (session, edit) = with_laser();
    let _ = session;
    let mut fresh = Session::new(3);
    fresh.load_formats(&edit.file_text).unwrap();
    assert_eq!(groups(&fresh), ["Paper", "Laser"]);
    let orientation = fresh.format_list().groups[1].orientation;
    assert_eq!(orientation, "landscape");
    let _ = Orientation::Landscape;
}

/// An empty or blank file means no formats of the maker's, not a broken file.
#[test]
fn an_empty_file_is_no_formats_and_not_a_broken_file() {
    for text in ["", "  \n\t\n"] {
        let mut session = Session::new(1);
        assert!(session.load_formats(text).is_ok(), "{text:?}");
        assert_eq!(session.formats_broken(), None);
        assert_eq!(groups(&session), ["Paper"]);
        assert!(session.set_format_favourite("a4", false).ok);
    }
}
