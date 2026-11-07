# pocketgen-editor

A `no_std` + `alloc` editing core for pocketgen. Its gap buffer, document
traits, and word navigation are derived from [Microsoft Edit](https://github.com/microsoft/edit)
at commit `b7f83220868373945945e2218e852c24cbb4dcf0`. The original MIT license
is included in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
The pocketgen-editor component is available under the [MIT license](LICENSE).

- Heap-backed gap buffer with byte-offset edits and revision tracking.
- UTF-8 editing, cursor movement, selection, clipboard, and bounded undo history.
- UART key decoding, menu navigation, and character display-width calculation.
- Independent allocation-free Python lexer for experimental syntax highlighting.

Run the tests from the repository root:

```sh
cargo test --manifest-path components/pocketgen-editor/Cargo.toml --target-dir build/editor-tests
```