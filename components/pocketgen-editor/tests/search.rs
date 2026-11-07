use pocketgen_editor::{Decoder, Editor, Key, Search, SearchStatus};

fn type_query(search: &mut Search, editor: &mut Editor, query: &str) {
    for ch in query.chars() {
        assert!(search.handle(Key::Character(ch), editor));
    }
}

#[test]
fn finds_unicode_matches_and_wraps_without_editing() {
    let mut editor = Editor::new("one 日\n日 one");
    let mut search = Search::default();
    assert_eq!(Decoder::default().feed(0x06), Ok(Some(Key::Find)));
    assert!(!search.handle(Key::Character('x'), &mut editor));
    assert!(search.handle(Key::Find, &mut editor));
    type_query(&mut search, &mut editor, "日");
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(4..7));
    assert_eq!(editor.cursor(), 4);
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(8..11));
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(4..7));
    search.handle(Key::Escape, &mut editor);
    assert!(!search.active());
    assert_eq!(editor.selection(), Some(4..7));
    editor.apply(Key::Undo);
    assert_eq!(editor.text(), "one 日\n日 one");
}

#[test]
fn starts_at_cursor_and_wraps_across_cursor_boundary() {
    let mut editor = Editor::new("abc xyz abc");
    editor.apply(Key::Right);
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    type_query(&mut search, &mut editor, "abc");
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(8..11));
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(0..3));
    search.handle(Key::Escape, &mut editor);
    let mut only = Editor::new("abc");
    only.apply(Key::Right);
    search.handle(Key::Find, &mut only);
    type_query(&mut search, &mut only, "abc");
    search.handle(Key::Enter, &mut only);
    assert_eq!(only.selection(), Some(0..3));
}

#[test]
fn handles_empty_missing_case_and_modal_keys() {
    let mut editor = Editor::new("Hello");
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    search.handle(Key::Enter, &mut editor);
    search.handle(Key::FindNext, &mut editor);
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), None);
    type_query(&mut search, &mut editor, "hello");
    search.handle(Key::Enter, &mut editor);
    assert_eq!(search.status(), SearchStatus::NotFound);
    assert_eq!(editor.cursor(), 0);
    search.handle(Key::FindNext, &mut editor);
    assert_eq!(search.status(), SearchStatus::NotFound);
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(search.status(), SearchStatus::NotFound);
    search.handle(Key::Run, &mut editor);
    search.handle(Key::Menu, &mut editor);
    assert!(search.active());
    assert_eq!(editor.text(), "Hello");
    for _ in 0..5 {
        search.handle(Key::Backspace, &mut editor);
    }
    type_query(&mut search, &mut editor, "Hello");
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(0..5));
    search.handle(Key::Escape, &mut editor);
    search.handle(Key::Find, &mut editor);
    assert_eq!(search.query(), "Hello");
    assert_eq!(search.status(), SearchStatus::Ready);
}

#[test]
fn prefills_only_single_line_selections_within_byte_limit() {
    for (text, expected) in [
        ("abc", "abc"),
        ("日\t界", "日\t界"),
        ("abc\ndef", ""),
        ("abc\rdef", ""),
        (&"x".repeat(256), &"x".repeat(256)),
        (&"x".repeat(257), ""),
        (&"日".repeat(85), &"日".repeat(85)),
        (&"日".repeat(86), ""),
    ] {
        let mut editor = Editor::new(text);
        editor.apply(Key::SelectAll);
        let mut search = Search::default();
        search.handle(Key::Find, &mut editor);
        assert_eq!(search.query(), expected);
        assert_eq!(search.status(), SearchStatus::Ready);
        assert_eq!(editor.selection(), Some(0..text.len()));
    }

    let mut editor = Editor::new("one two");
    editor.apply(Key::End);
    for _ in 0..3 {
        editor.apply(Key::ShiftLeft);
    }
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    assert_eq!(search.query(), "two");
}

#[test]
fn searches_in_both_directions_from_a_prefilled_selection() {
    let mut editor = Editor::new("日 one 日 two 日");
    editor.apply(Key::CtrlEnd);
    editor.apply(Key::ShiftLeft);
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    assert_eq!(search.query(), "日");
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(8..11));
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(0..3));
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(16..19));
    search.handle(Key::FindNext, &mut editor);
    assert_eq!(editor.selection(), Some(0..3));
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(8..11));
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(0..3));
}

#[test]
fn reverse_search_handles_overlap_empty_and_missing_queries() {
    let mut editor = Editor::new("banana");
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), None);
    type_query(&mut search, &mut editor, "ana");
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(3..6));
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(1..4));
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(editor.selection(), Some(3..6));
    search.handle(Key::Backspace, &mut editor);
    type_query(&mut search, &mut editor, "xyz");
    search.handle(Key::FindPrevious, &mut editor);
    assert_eq!(search.status(), SearchStatus::NotFound);
    assert_eq!(editor.selection(), Some(3..6));
}

#[test]
fn enforces_byte_limit_and_backspaces_unicode() {
    let mut editor = Editor::new("");
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    type_query(&mut search, &mut editor, &"日".repeat(85));
    type_query(&mut search, &mut editor, "界");
    assert_eq!(search.query().len(), 255);
    assert_eq!(search.status(), SearchStatus::QueryTooLong);
    search.handle(Key::Backspace, &mut editor);
    assert_eq!(search.query().len(), 252);
    assert_eq!(search.status(), SearchStatus::Ready);
    type_query(&mut search, &mut editor, "界");
    assert_eq!(search.query().len(), 255);
}

#[test]
fn tolerates_document_changes_between_searches() {
    let mut editor = Editor::new("long match");
    let mut search = Search::default();
    search.handle(Key::Find, &mut editor);
    type_query(&mut search, &mut editor, "match");
    search.handle(Key::Enter, &mut editor);
    editor.replace_all("match");
    search.handle(Key::Enter, &mut editor);
    assert_eq!(editor.selection(), Some(0..5));
}
