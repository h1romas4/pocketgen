use pocketgen_editor::syntax::{Highlighter, Kind};

fn tokens<'a>(highlighter: &mut Highlighter, line: &'a str) -> Vec<(&'a str, Kind)> {
    let spans: Vec<_> = highlighter.line(line).collect();
    let mut end = 0;
    for span in &spans {
        assert_eq!(span.start, end);
        assert!(span.end > span.start);
        assert!(line.is_char_boundary(span.start));
        assert!(line.is_char_boundary(span.end));
        end = span.end;
    }
    assert_eq!(end, line.len());
    spans
        .into_iter()
        .map(|span| (&line[span.start..span.end], span.kind))
        .collect()
}

#[test]
fn colors_python_tokens_without_misclassifying_identifiers() {
    let mut highlighter = Highlighter::default();
    let got = tokens(
        &mut highlighter,
        "if iffy 変数if 0xff+1 1.5e-2 .25 # comment",
    );
    assert!(got.contains(&("if", Kind::Keyword)));
    assert!(got.contains(&("iffy", Kind::Plain)));
    assert!(got.contains(&("変数if", Kind::Plain)));
    assert!(got.contains(&("0xff", Kind::Number)));
    assert!(got.contains(&("+", Kind::Plain)));
    assert!(got.contains(&("1.5e-2", Kind::Number)));
    assert!(got.contains(&(".25", Kind::Number)));
    assert!(got.contains(&("# comment", Kind::Comment)));
}

#[test]
fn handles_strings_comments_prefixes_and_escapes() {
    let mut highlighter = Highlighter::default();
    let got = tokens(
        &mut highlighter,
        r##"x = r"not # a comment" + f'{value}' + "a\"b" # real"##,
    );
    assert!(got.contains(&(r##"r"not # a comment""##, Kind::String)));
    assert!(got.contains(&("f'{value}'", Kind::String)));
    assert!(got.contains(&(r##""a\"b""##, Kind::String)));
    assert!(got.contains(&("# real", Kind::Comment)));
}

#[test]
fn retains_multiline_state_and_recovers_after_closing() {
    let mut highlighter = Highlighter::default();
    assert_eq!(
        tokens(&mut highlighter, "s = '''hello #"),
        [
            ("s", Kind::Plain),
            (" ", Kind::Plain),
            ("=", Kind::Plain),
            (" ", Kind::Plain),
            ("'''hello #", Kind::String)
        ]
    );
    assert_eq!(
        tokens(&mut highlighter, "still string''' # real"),
        [
            ("still string'''", Kind::String),
            (" ", Kind::Plain),
            ("# real", Kind::Comment)
        ]
    );
    assert!(tokens(&mut highlighter, "return 3").contains(&("return", Kind::Keyword)));
}

#[test]
fn single_quote_continuation_does_not_leak_into_later_lines() {
    let mut highlighter = Highlighter::default();
    assert_eq!(
        tokens(&mut highlighter, "'continued\\"),
        [("'continued\\", Kind::String)]
    );
    assert_eq!(
        tokens(&mut highlighter, "end' # comment"),
        [
            ("end'", Kind::String),
            (" ", Kind::Plain),
            ("# comment", Kind::Comment)
        ]
    );
    tokens(&mut highlighter, "'unterminated");
    assert!(tokens(&mut highlighter, "if x:").contains(&("if", Kind::Keyword)));
    tokens(&mut highlighter, "'continued\\");
    assert!(tokens(&mut highlighter, "").is_empty());
    assert!(tokens(&mut highlighter, "if x:").contains(&("if", Kind::Keyword)));
}

#[test]
fn highlighter_handles_empty_lines_and_triple_quote_escapes() {
    let mut highlighter = Highlighter::default();
    assert!(tokens(&mut highlighter, "").is_empty());
    tokens(&mut highlighter, r#"r"""quoted \" still open"#);
    assert!(tokens(&mut highlighter, "").is_empty());
    assert_eq!(
        tokens(&mut highlighter, r#"end""" if True"#),
        [
            (r#"end""""#, Kind::String),
            (" ", Kind::Plain),
            ("if", Kind::Keyword),
            (" ", Kind::Plain),
            ("True", Kind::Keyword)
        ]
    );
}
