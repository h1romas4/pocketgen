# pocketgen-terminal

A `no_std` + `alloc` terminal renderer derived from
[embedded-graphics-terminal](https://github.com/second-state/embedded-graphics-terminal)
at commit `2018436ae9040e12ab490ebc7420667ab5a4ccf1`. The original MIT license
is included in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
The pocketgen-terminal component is available under the [MIT license](LICENSE).

- Renders `vt100` screens to `embedded-graphics` RGB565 targets, including
	colors, themes, cursor, and incremental updates.
- Supports CJK fonts, fallback fonts, character substitutions, and glyph caching.
- Provides bounded RGB565 buffer regions and newline-aware terminal output.

Zephyr requires `CONFIG_RUST_ALLOC=y` and a suitable C allocator. The optional
`timing` feature requires `std`.

Run the tests from the repository root:

```sh
cargo test --manifest-path components/pocketgen-terminal/Cargo.toml --target-dir build/terminal-tests
```