//! Runtime integration checks enabled by the application's self-test setting.
//!
//! These checks run in the Zephyr application, not under Rust's test harness.
//! Each returns `false` when the tested behavior or display geometry fails.

use alloc::{format, vec};
use embedded_graphics::geometry::Size;
use pocketgen_editor::{Editor, Key, Menu, Search, SearchStatus};
use pocketgen_terminal::{vt100, Rgb565Surface, TerminalRenderer};

use crate::apply_input;
use crate::output_pane::OutputPane;
use crate::python::{execute, LiveDisplay};
use crate::ui::{draw_running, draw_shortcuts, terminal, Bar};
use crate::viewport::{pocketgen_syntax_highlight_enabled, Viewport};

fn run_graphics_source(
    source: &str,
    renderer: &mut TerminalRenderer,
    parser: &mut vt100::Parser,
    pane: &mut OutputPane,
    pixels: &mut [u16],
    width: u16,
    height: u16,
    bar_height: u16,
) -> i32 {
    let mut live = LiveDisplay {
        renderer,
        pane,
        pixels,
        width,
        height,
        bar_height,
        error: 0,
        ignored_input: false,
        self_test: false,
    };
    execute(&Editor::new(source), parser, Some(&mut live)).status
}

/// Checks graphics primitives, retention, text switching, and graphics demos.
pub(crate) fn test_graphics(
    renderer: &mut TerminalRenderer,
    parser: &mut vt100::Parser,
    pane: &mut OutputPane,
    pixels: &mut [u16],
    width: u16,
    height: u16,
    bar_height: u16,
) -> bool {
    if pane.toggle() {
        return false;
    }
    let source = "from pocketgen import graphics_size, graphics_clear, graphics_pixel, graphics_line, graphics_show, refresh\n\
        w, h = graphics_size()\n\
        assert w >= 4 and h >= 4\n\
        graphics_clear(0x1234)\n\
        graphics_pixel(0, 0, 0xffff)\n\
        graphics_line(1, 1, 3, 3, 0xf800)\n\
        graphics_line(0, h - 1, w - 1, h - 1, 0x07e0)\n\
        graphics_line(w - 1, 0, w - 1, h - 1, 0x001f)\n\
        print('text-visible')\n\
        graphics_show()\n\
        refresh()\n";
    if run_graphics_source(
        source, renderer, parser, pane, pixels, width, height, bar_height,
    ) != 0
        || !parser.screen().contents().contains("text-visible")
    {
        return false;
    }
    let x = width / 2;
    let index = usize::from(bar_height) * usize::from(width) + usize::from(x);
    for _ in 0..2 {
        pixels.fill(0x55aa);
        pane.render(parser, renderer, pixels, width, x, bar_height);
        if pixels[index] != 0xffff
            || pixels[index + 1] != 0x1234
            || pixels[index + usize::from(width) + 1] != 0xf800
            || pixels[index + 3 * usize::from(width) + 3] != 0xf800
            || pixels[index + usize::from(width - x) - 1] != 0x001f
            || pixels[usize::from(height - bar_height - 1) * usize::from(width) + usize::from(x)]
                != 0x07e0
            || pixels
                [usize::from(height - bar_height - 1) * usize::from(width) + usize::from(width - 1)]
                != 0x001f
            || pixels[index - 1] != 0x55aa
        {
            return false;
        }
    }
    if !pane.toggle() || pane.graphics_command(3, 0, 0, 0, 0, 0x07e0) != 0 {
        return false;
    }
    pane.render(parser, renderer, pixels, width, x, bar_height);
    if pixels[index] == 0x07e0 || !parser.screen().contents().contains("text-visible") {
        return false;
    }
    if !pane.toggle() {
        return false;
    }
    pane.show_text();
    pane.render(parser, renderer, pixels, width, x, bar_height);
    if pixels[index] != 0x07e0 {
        return false;
    }
    pane.show_error();
    pane.render(parser, renderer, pixels, width, x, bar_height);
    if pixels[index] == 0x07e0 || !pane.toggle() {
        return false;
    }
    let invalid = concat!(
        "from pocketgen import graphics_size, graphics_clear, graphics_pixel, graphics_line\n",
        "w, h = graphics_size()\n",
        "graphics_clear()\n",
        "for f, args in ((graphics_pixel, (-1, 0, 1)), (graphics_pixel, (w, 0, 1)),\n",
        "                (graphics_pixel, (0, h, 1)), (graphics_pixel, (0, 0, -1)),\n",
        "                (graphics_pixel, (0, 0, 65536)),\n",
        "                (graphics_line, (0, 0, w, 0, 1))):\n",
        " try:\n",
        "  f(*args)\n",
        "  raise RuntimeError('invalid graphics accepted')\n",
        " except ValueError:\n",
        "  pass\n",
        "raise ValueError('visible-error')\n",
    );
    if run_graphics_source(
        invalid, renderer, parser, pane, pixels, width, height, bar_height,
    ) != 1
        || !parser.screen().contents().contains("visible-error")
    {
        return false;
    }
    pane.render(parser, renderer, pixels, width, x, bar_height);
    if pixels[index] == 0x1234 {
        return false;
    }
    let before_clear = concat!(
        "from pocketgen import graphics_pixel, graphics_line, graphics_show\n",
        "for f, args in ((graphics_pixel, (0, 0, 1)),\n",
        "                (graphics_line, (0, 0, 1, 1, 1)), (graphics_show, ())):\n",
        " try:\n",
        "  f(*args)\n",
        "  raise RuntimeError('uninitialized graphics accepted')\n",
        " except OSError:\n",
        "  pass\n",
    );
    if run_graphics_source(
        before_clear,
        renderer,
        parser,
        pane,
        pixels,
        width,
        height,
        bar_height,
    ) != 0
    {
        return false;
    }
    if run_graphics_source(
        include_str!("../examples/sine_graph.py"),
        renderer,
        parser,
        pane,
        pixels,
        width,
        height,
        bar_height,
    ) != 0
    {
        log::error!("Sine graph demo failed: {}", parser.screen().contents());
        return false;
    }
    pixels.fill(0);
    pane.render(parser, renderer, pixels, width, x, bar_height);
    let curve_pixels = (usize::from(bar_height)..usize::from(height - bar_height))
        .flat_map(|row| {
            &pixels[row * usize::from(width) + usize::from(x)..(row + 1) * usize::from(width)]
        })
        .filter(|&&pixel| pixel == 0x07ff)
        .count();
    let graph_width = usize::from(width - x);
    let graph_height = usize::from(height - 2 * bar_height);
    let middle = graph_height / 2;
    let amplitude = (graph_height - 2) * 2 / 5;
    let peaks = [
        (1, middle - amplitude),
        (3, middle + amplitude),
        (5, middle - amplitude),
        (7, middle + amplitude),
    ];
    if curve_pixels < graph_width / 2
        || !peaks.into_iter().all(|(eighth, expected_y)| {
            let column = (graph_width - 1) * eighth / 8;
            let start = expected_y.saturating_sub(4);
            let end = (expected_y + 4).min(graph_height - 1);
            (start..=end).any(|row| {
                pixels
                    [(usize::from(bar_height) + row) * usize::from(width) + usize::from(x) + column]
                    == 0x07ff
            })
        })
    {
        return false;
    }

    let Some(definitions) =
        include_str!("../examples/mexican_hat_graphics.py").strip_suffix("run_demo()\n")
    else {
        log::error!("Graphics Mexican hat demo entry point is missing");
        return false;
    };
    let source = format!("{}run_demo(12)\n", definitions);
    if run_graphics_source(
        &source, renderer, parser, pane, pixels, width, height, bar_height,
    ) != 0
    {
        log::error!(
            "Graphics Mexican hat demo failed: {}",
            parser.screen().contents()
        );
        return false;
    }
    pixels.fill(0);
    pane.render(parser, renderer, pixels, width, x, bar_height);
    let graphic_pixels =
        (usize::from(bar_height)..usize::from(height - bar_height)).flat_map(|row| {
            &pixels[row * usize::from(width) + usize::from(x)..(row + 1) * usize::from(width)]
        });
    let mut count = 0;
    let mut blue = false;
    let mut red = false;
    for &pixel in graphic_pixels {
        if pixel != 0 {
            count += 1;
        }
        blue |= pixel == 0x001f;
        red |= pixel == 0xf800;
    }
    count > graph_width * 2 && blue && red
}

/// Checks search input, selection, scrolling, and prompt rendering.
pub(crate) fn test_search(size: Size) -> bool {
    let Some((_, mut parser)) = terminal(size) else {
        return false;
    };
    let mut editor = Editor::new("zero\none\n日\nlast 日");
    let mut search = Search::default();
    let mut viewport = Viewport { top: 0, left: 0 };
    search.handle(Key::Find, &mut editor);
    search.handle(Key::Character('日'), &mut editor);
    search.handle(Key::Enter, &mut editor);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 24);
    viewport.render_search(&search, &mut parser, 24);
    if editor.selection() != Some(9..12)
        || viewport.top != 1
        || !parser.screen().contents().contains("Find: 日")
        || !parser
            .screen()
            .cell(2, 3)
            .is_some_and(|cell| cell.inverse())
    {
        return false;
    }
    search.handle(Key::Enter, &mut editor);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 24);
    if editor.selection() != Some(18..21) || viewport.top != 2 {
        return false;
    }
    search.handle(Key::FindPrevious, &mut editor);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 24);
    if editor.selection() != Some(9..12) || viewport.top != 2 {
        return false;
    }
    search.handle(Key::Backspace, &mut editor);
    search.handle(Key::Character('z'), &mut editor);
    search.handle(Key::Character('z'), &mut editor);
    search.handle(Key::Enter, &mut editor);
    viewport.render_search(&search, &mut parser, 24);
    if search.status() != SearchStatus::NotFound
        || !parser.screen().contents().contains("[Not found]")
    {
        return false;
    }
    search.handle(Key::Escape, &mut editor);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 24);
    if search.active() || !parser.screen().contents().contains("Demo  Edit  Run") {
        return false;
    }
    let mut editor = Editor::new("日\t日");
    editor.apply(Key::SelectAll);
    search.handle(Key::Find, &mut editor);
    viewport.render_search(&search, &mut parser, 24);
    search.query() == "日\t日" && parser.screen().contents().contains("Find: 日 日")
}

/// Checks Python output, exception reporting, and rendering within the pane.
pub(crate) fn test_terminal_bridge(width: u16, height: u16, x_offset: u16, size: Size) -> bool {
    let Some((mut renderer, mut parser)) = terminal(size) else {
        return false;
    };
    let editor = Editor::new("print('bridge-ok')\nraise ValueError('bad input')\n");
    let result = execute(&editor, &mut parser, None);
    let contents = parser.screen().contents();
    if result.status != 1
        || result.free_bytes.is_none_or(|bytes| bytes == 0)
        || !contents.contains("bridge-ok")
        || !contents.contains("ValueError")
    {
        return false;
    }

    let mut pixels = vec![0x4321; width as usize * height as usize];
    renderer
        .render(
            parser.screen(),
            &mut Rgb565Surface::new(&mut pixels, width, size, x_offset, 0).unwrap(),
        )
        .unwrap();
    pixels.chunks_exact(width as usize).all(|row| {
        row[..x_offset as usize]
            .iter()
            .all(|pixel| *pixel == 0x4321)
    }) && pixels.chunks_exact(width as usize).any(|row| {
        row[x_offset as usize..]
            .iter()
            .any(|pixel| *pixel != 0x4321)
    })
}

/// Checks viewport scrolling, cursor placement, and line-number gutters.
pub(crate) fn test_editor_viewport(size: Size) -> bool {
    let Some((_, mut parser)) = terminal(size) else {
        return false;
    };
    let mut editor = Editor::new("first\n0123456789\nlast");
    let mut viewport = Viewport { top: 0, left: 0 };
    editor.apply(Key::Down);
    editor.apply(Key::End);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if viewport.left != 6
        || parser.screen().cursor_position() != (2, 7)
        || !parser.screen().contents().contains(" 2 6789")
    {
        return false;
    }
    editor.apply(Key::Down);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if viewport.top != 1
        || parser.screen().cursor_position() != (2, 3)
        || parser
            .screen()
            .cell(2, 1)
            .is_none_or(|cell| cell.contents() != "3")
    {
        return false;
    }
    editor.apply(Key::CtrlHome);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if viewport.top != 0 || viewport.left != 0 || parser.screen().cursor_position() != (1, 3) {
        return false;
    }
    editor.apply(Key::CtrlEnd);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if viewport.top != 1 || parser.screen().cursor_position() != (2, 7) {
        return false;
    }

    let text = format!("{}x", "x\n".repeat(9));
    let mut editor = Editor::new(&text);
    let mut viewport = Viewport { top: 0, left: 0 };
    let old = editor.cursor();
    editor.apply_with_page_height(Key::PageDown, 2);
    viewport.scroll_page(&text, old, editor.cursor(), 3);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if editor.cursor() != 4 || viewport.top != 2 || parser.screen().cursor_position() != (1, 3) {
        return false;
    }
    let old = editor.cursor();
    editor.apply_with_page_height(Key::ShiftPageDown, 2);
    viewport.scroll_page(&text, old, editor.cursor(), 3);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if editor.selection() != Some(4..8)
        || viewport.top != 4
        || parser.screen().cursor_position() != (1, 3)
    {
        return false;
    }
    let old = editor.cursor();
    editor.apply_with_page_height(Key::PageUp, 20);
    viewport.scroll_page(&text, old, editor.cursor(), 3);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if editor.cursor() != 0 || editor.selection().is_some() || viewport.top != 0 {
        return false;
    }

    let text = "\n".repeat(99);
    let mut editor = Editor::new(&text);
    let mut viewport = Viewport { top: 0, left: 0 };
    for _ in 0..99 {
        editor.apply(Key::Down);
    }
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 8);
    if viewport.top != 98
        || parser.screen().cursor_position() != (2, 4)
        || parser
            .screen()
            .cell(1, 1)
            .is_none_or(|cell| cell.contents() != "9")
        || parser
            .screen()
            .cell(2, 0)
            .is_none_or(|cell| cell.contents() != "1")
        || parser
            .screen()
            .cell(2, 2)
            .is_none_or(|cell| cell.contents() != "0")
    {
        return false;
    }
    editor.apply(Key::Up);
    viewport.render(&editor, &Menu::default(), &mut parser, 3, 2);
    parser.screen().cursor_position() == (1, 1)
        && parser
            .screen()
            .cell(1, 0)
            .is_some_and(|cell| cell.contents() == "9")
}

/// Checks token colors, multiline scrolling, selection, and the plain mode.
pub(crate) fn test_syntax_highlighting(size: Size) -> bool {
    let Some((_, mut parser)) = terminal(size) else {
        return false;
    };
    let mut editor = Editor::new("msg = '''hello\nif not keyword\nend''' # note\nif True: x=42");
    editor.apply(Key::Down);
    editor.apply(Key::ShiftRight);
    editor.apply(Key::ShiftRight);
    let mut viewport = Viewport { top: 1, left: 0 };
    viewport.render_with_highlight(&editor, &Menu::default(), &mut parser, 4, 24, true);
    let colored = [(1, 3, 11), (2, 3, 11), (2, 10, 10), (3, 3, 14), (3, 14, 13)]
        .into_iter()
        .all(|(row, col, color)| {
            parser
                .screen()
                .cell(row, col)
                .is_some_and(|cell| cell.fgcolor() == vt100::Color::Idx(color))
        });
    if !colored
        || !parser
            .screen()
            .cell(1, 3)
            .is_some_and(|cell| cell.inverse())
    {
        return false;
    }
    viewport.render_with_highlight(&editor, &Menu::default(), &mut parser, 4, 24, false);
    if !parser.screen().cell(1, 3).is_some_and(|cell| {
        cell.fgcolor() == vt100::Color::Default && cell.inverse() && cell.contents() == "i"
    }) {
        return false;
    }
    viewport.render(&editor, &Menu::default(), &mut parser, 4, 24);
    let expected = if unsafe { pocketgen_syntax_highlight_enabled() } != 0 {
        vt100::Color::Idx(11)
    } else {
        vt100::Color::Default
    };
    if !parser
        .screen()
        .cell(1, 3)
        .is_some_and(|cell| cell.fgcolor() == expected && cell.inverse())
    {
        return false;
    }

    let mut editor = Editor::new("x = '''01234567890123456789\n# still a string\nend'''");
    editor.apply(Key::Down);
    let mut viewport = Viewport { top: 0, left: 0 };
    viewport.render_with_highlight(&editor, &Menu::default(), &mut parser, 4, 12, true);
    parser
        .screen()
        .cell(2, 3)
        .is_some_and(|cell| cell.contents() == "#" && cell.fgcolor() == vt100::Color::Idx(11))
}

/// Checks selection highlighting and menu navigation on the terminal screen.
pub(crate) fn test_selection_and_menu(size: Size) -> bool {
    let Some((renderer, mut parser)) = terminal(size) else {
        return false;
    };
    let mut editor = Editor::new("a日\nnext");
    let mut menu = Menu::default();
    let mut viewport = Viewport { top: 0, left: 0 };
    editor.apply(Key::ShiftRight);
    editor.apply(Key::ShiftRight);
    editor.apply(Key::ShiftRight);
    viewport.render(
        &editor,
        &menu,
        &mut parser,
        renderer.rows(),
        renderer.cols(),
    );
    if ![3, 4, 6].into_iter().all(|col| {
        parser
            .screen()
            .cell(1, col)
            .is_some_and(|cell| cell.inverse())
    }) || parser
        .screen()
        .cell(1, 1)
        .is_none_or(|cell| cell.inverse() || cell.contents() != "1")
    {
        return false;
    }
    menu.handle(Key::Menu);
    menu.handle(Key::Down);
    menu.handle(Key::Down);
    viewport.render(
        &editor,
        &menu,
        &mut parser,
        renderer.rows(),
        renderer.cols(),
    );
    if !parser.screen().contents().contains("Copy")
        || !parser
            .screen()
            .cell(3, 6)
            .is_some_and(|cell| cell.bgcolor() == vt100::Color::Idx(4))
    {
        return false;
    }
    if menu.handle(Key::Enter) != Some(Key::Copy) {
        return false;
    }
    editor.apply(Key::Copy);
    viewport.render(
        &editor,
        &menu,
        &mut parser,
        renderer.rows(),
        renderer.cols(),
    );
    parser
        .screen()
        .cell(1, 3)
        .is_some_and(|cell| cell.inverse())
}

/// Checks that demo menu commands replace the buffer without starting Python.
pub(crate) fn test_demo_loading(size: Size, footer_size: Size) -> bool {
    let Some((_, mut right)) = terminal(size) else {
        return false;
    };
    let Some(mut footer) = Bar::new(footer_size, 0, 0) else {
        return false;
    };
    let mut editor = Editor::new("unsaved\n");
    let mut menu = Menu::default();
    for key in [Key::Menu, Key::Left, Key::Down, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    if editor.text() != include_str!("../examples/starfield.py")
        || editor.cursor() != 0
        || !right.screen().contents().trim().is_empty()
    {
        return false;
    }
    editor.apply(Key::Undo);
    if editor.text() != "unsaved\n" {
        return false;
    }
    for key in [Key::Menu, Key::Left, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    if editor.text() != include_str!("../examples/terminal_demo.py")
        || editor.cursor() != 0
        || !right.screen().contents().trim().is_empty()
    {
        return false;
    }
    for key in [Key::Menu, Key::Left, Key::Down, Key::Down, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    if editor.text() != include_str!("../examples/cube.py")
        || editor.cursor() != 0
        || !right.screen().contents().trim().is_empty()
    {
        return false;
    }
    for key in [Key::Menu, Key::Left, Key::Up, Key::Up, Key::Up, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    if editor.text() != include_str!("../examples/mexican_hat.py")
        || editor.cursor() != 0
        || !right.screen().contents().trim().is_empty()
    {
        return false;
    }
    for key in [Key::Menu, Key::Left, Key::Up, Key::Up, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    if editor.text() != include_str!("../examples/sine_graph.py")
        || editor.cursor() != 0
        || !right.screen().contents().trim().is_empty()
    {
        return false;
    }
    for key in [Key::Menu, Key::Left, Key::Up, Key::Enter] {
        if !apply_input(
            key,
            &mut menu,
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return false;
        }
    }
    editor.text() == include_str!("../examples/mexican_hat_graphics.py")
        && editor.cursor() == 0
        && right.screen().contents().trim().is_empty()
}

pub(crate) fn test_demo_execution(size: Size, source: &str, title: &str) -> bool {
    let Some((_, mut right)) = terminal(size) else {
        return false;
    };
    let Some(definitions) = source.strip_suffix("run_demo()\n") else {
        log::error!("Demo entry point is missing: {}", title);
        return false;
    };
    let editor = Editor::new(&format!("{}run_demo(4, 0)\n", definitions));
    let result = execute(&editor, &mut right, None);
    if result.status != 0 {
        log::error!("{} demo failed: {}", title, right.screen().contents());
        return false;
    }
    let contents = right.screen().contents();
    contents.contains(title)
        && (contents.contains('*') || contents.contains('+') || contents.contains('.'))
}

pub(crate) fn test_terminal_controls(size: Size) -> bool {
    let Some((_, mut right)) = terminal(size) else {
        return false;
    };
    let unavailable = Editor::new("from pocketgen import graphics_size\ngraphics_size()\n");
    if execute(&unavailable, &mut right, None).status != 1
        || !right.screen().contents().contains("OSError")
    {
        return false;
    }
    let editor = Editor::new(
        "from pocketgen import cls, locate, color\n\
         print('discard')\n\
         cls()\n\
         locate(3, 2)\n\
         color(10, 4)\n\
         print('X')\n",
    );
    if execute(&editor, &mut right, None).status != 0
        || right.screen().contents().contains("discard")
        || !right.screen().cell(2, 3).is_some_and(|cell| {
            cell.contents() == "X"
                && cell.fgcolor() == vt100::Color::Idx(10)
                && cell.bgcolor() == vt100::Color::Idx(4)
        })
    {
        return false;
    }

    let editor = Editor::new(
        "from pocketgen import locate, color\nfor i in range(16):\n \
         locate(i, 0)\n color(i, 15-i)\n print('X')\n",
    );
    if execute(&editor, &mut right, None).status != 0
        || !(0..16).all(|i| {
            right.screen().cell(0, i).is_some_and(|cell| {
                cell.contents() == "X"
                    && cell.fgcolor() == vt100::Color::Idx(i as u8)
                    && cell.bgcolor() == vt100::Color::Idx((15 - i) as u8)
            })
        })
    {
        return false;
    }

    let (rows, cols) = right.screen().size();
    let source = format!(
        "from pocketgen import locate\nlocate({}, {})\n",
        cols.get() - 1,
        rows.get() - 1
    );
    if execute(&Editor::new(&source), &mut right, None).status != 0
        || right.screen().cursor_position() != (rows.get() - 1, cols.get() - 1)
    {
        return false;
    }
    let source = format!("from pocketgen import locate\nlocate({}, 0)\n", cols.get());
    if execute(&Editor::new(&source), &mut right, None).status != 1
        || !right.screen().contents().contains("ValueError")
    {
        return false;
    }

    let editor = Editor::new("print('reset')\n");
    execute(&editor, &mut right, None).status == 0
        && right.screen().cell(0, 0).is_some_and(|cell| {
            cell.contents() == "r"
                && cell.fgcolor() == vt100::Color::Default
                && cell.bgcolor() == vt100::Color::Default
        })
}

/// Checks heap status updates for successful runs and Python exceptions.
pub(crate) fn test_heap_status(right_size: Size, footer_size: Size) -> bool {
    let Some((_, mut right)) = terminal(right_size) else {
        return false;
    };
    let Some(mut footer) = Bar::new(footer_size, 0, 0) else {
        return false;
    };
    draw_running(&mut footer);
    if !footer.parser.screen().contents().contains("Ctrl+C Stop")
        || !footer.parser.screen().contents().contains("Ctrl+G View")
    {
        return false;
    }
    draw_shortcuts(&mut footer, None);
    if !footer.parser.screen().contents().ends_with("MP free: --") {
        return false;
    }

    let mut run = |source: &str| {
        let mut editor = Editor::new(source);
        if !apply_input(
            Key::Run,
            &mut Menu::default(),
            &mut editor,
            &mut right,
            &mut footer,
            None,
            false,
            1,
        ) {
            return None;
        }
        let contents = footer.parser.screen().contents();
        let bytes = contents.rsplit("MP free: ").next()?.strip_suffix(" B")?;
        Some((bytes.parse::<usize>().ok()?, right.screen().contents()))
    };
    let (Some((baseline, _)), Some((allocated, _)), Some((reset, _))) =
        (run("pass\n"), run("x = [0] * 2048\n"), run("pass\n"))
    else {
        return false;
    };
    if baseline <= allocated || baseline - allocated < 7000 || baseline != reset {
        return false;
    }
    let Some((runtime_free, runtime_output)) = run("raise ValueError('bad input')\n") else {
        return false;
    };
    let Some((syntax_free, syntax_output)) = run("if True print(1)\n") else {
        return false;
    };
    runtime_free > 0
        && syntax_free > 0
        && runtime_output.contains("ValueError")
        && syntax_output.contains("SyntaxError")
}
