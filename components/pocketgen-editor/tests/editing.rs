use pocketgen_editor::document::ReadableDocument;
use pocketgen_editor::GapBuffer;

fn contents(buffer: &GapBuffer) -> String {
    let mut result = String::new();
    buffer.copy_into(&mut result);
    result
}

#[test]
fn editing_across_gap_and_growth() {
    let mut buffer = GapBuffer::new();
    let mut expected = String::new();
    let mut seed = 7_u32;
    for step in 0..1000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let start = seed as usize % (expected.len() + 1);
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let end = start + seed as usize % (expected.len() - start + 1);
        let replacement = if step % 3 == 0 {
            "abcdefghijklmnop"
        } else {
            "xy"
        };
        buffer.replace(start..end, replacement.as_bytes());
        expected.replace_range(start..end, replacement);
        assert_eq!(contents(&buffer), expected);
        assert_eq!(buffer.len(), expected.len());
    }
}

#[test]
fn unicode_copy_extract_clear_and_generation() {
    let mut buffer = GapBuffer::new();
    buffer.replace(0..0, "Hello 世界".as_bytes());
    let generation = buffer.generation();
    buffer.replace(6..6, "小".as_bytes());
    assert_eq!(contents(&buffer), "Hello 小世界");
    assert_eq!(buffer.read_forward(0), b"Hello \xe5\xb0\x8f");
    let mut extracted = b"[]".to_vec();
    buffer.extract_raw(6..buffer.len(), &mut extracted, 1);
    assert_eq!(extracted, "[小世界]".as_bytes());
    buffer.set_generation(generation);
    buffer.replace(0..5, b"Hi");
    assert_ne!(buffer.generation(), generation);
    assert_eq!(contents(&buffer), "Hi 小世界");
    assert!(!buffer.copy_from(&"Hi 小世界".as_bytes()));
    assert!(buffer.copy_from(&"".as_bytes()));
    assert!(buffer.is_empty());
    buffer.replace(0..0, b"again");
    buffer.clear();
    assert_eq!(contents(&buffer), "");
}

#[test]
fn grows_and_preserves_both_sides_of_gap() {
    let mut buffer = GapBuffer::new();
    buffer.replace(0..0, b"left-right");
    let insertion = "x".repeat(8192);
    buffer.replace(5..5, insertion.as_bytes());
    assert_eq!(contents(&buffer), format!("left-{insertion}right"));
    buffer.replace(4..8198, b" ");
    assert_eq!(contents(&buffer), "left ight");
}
