#![no_std]

//! PocketGen's Zephyr application entry point and event loop.
//!
//! The C bridge supplies the display, UART input, and MicroPython runtime.
//! Rust owns the editor, terminal state, framebuffer, and presentation logic.
//! At startup the application optionally runs integration checks, renders the
//! editor and Python output panes, then processes decoded UART input.

extern crate alloc;

#[cfg(target_arch = "riscv32")]
mod allocator;
mod display_buffer;
mod highlight_benchmark;
mod output_pane;
mod python;
mod self_test;
mod ui;
mod viewport;

use alloc::format;
use alloc::vec;
use core::ffi::{c_char, CStr};
use display_buffer::DisplayPixels;
use embedded_graphics::geometry::Size;
use embedded_graphics::pixelcolor::{raw::RawU16, Rgb565};
use embedded_graphics::prelude::RawData;
use log::{error, info, warn};
use output_pane::OutputPane;
use pocketgen_editor::{Decoder, Editor, Key, Menu, Search};
use pocketgen_terminal::vt100;
use pocketgen_terminal::TerminalRenderer;
use python::{execute, LiveDisplay};
use self_test::{
    test_demo_execution, test_demo_loading, test_editor_viewport, test_graphics, test_heap_status,
    test_search, test_selection_and_menu, test_syntax_highlighting, test_terminal_bridge,
    test_terminal_controls,
};
use ui::{blue_bar, draw_running, draw_shortcuts, render_display, terminal, Bar};
use viewport::Viewport;

extern "C" {
    fn pocketgen_display_init(width: *mut u16, height: *mut u16) -> i32;
    fn pocketgen_micropython_version() -> *const c_char;
    fn pocketgen_micropython_self_test() -> i32;
    fn pocketgen_input_init() -> i32;
    fn pocketgen_input_poll(byte: *mut u8) -> i32;
    fn pocketgen_input_wait();
    fn pocketgen_autostart_cube_enabled() -> i32;
}

/// Routes a key through the menu, editor, or Python execution path.
///
/// Returns `false` if execution fails or the optional UART self-test fails.
fn apply_input(
    key: Key,
    menu: &mut Menu,
    editor: &mut Editor,
    right: &mut vt100::Parser,
    footer: &mut Bar,
    live: Option<&mut LiveDisplay<'_>>,
    self_test: bool,
    page_lines: usize,
) -> bool {
    let Some(key) = menu.handle(key) else {
        return true;
    };
    match key {
        Key::DemoMaze => {
            editor.replace_all(include_str!("../examples/terminal_demo.py"));
            info!("Loaded maze demo");
            return true;
        }
        Key::DemoStarfield => {
            editor.replace_all(include_str!("../examples/starfield.py"));
            info!("Loaded starfield demo");
            return true;
        }
        Key::DemoCube => {
            editor.replace_all(include_str!("../examples/cube.py"));
            info!("Loaded cube demo");
            return true;
        }
        Key::DemoMexicanHat => {
            editor.replace_all(include_str!("../examples/mexican_hat.py"));
            info!("Loaded mexican hat demo");
            return true;
        }
        Key::DemoSineGraph => {
            editor.replace_all(include_str!("../examples/sine_graph.py"));
            info!("Loaded sine graph demo");
            return true;
        }
        Key::DemoMexicanHatGraphics => {
            editor.replace_all(include_str!("../examples/mexican_hat_graphics.py"));
            info!("Loaded mexican hat graphics demo");
            return true;
        }
        Key::DemoPlotTrig => {
            editor.replace_all(include_str!("../examples/plot_trig.py"));
            info!("Loaded plot trig demo");
            return true;
        }
        Key::DemoPlotProjectile => {
            editor.replace_all(include_str!("../examples/plot_projectile.py"));
            info!("Loaded plot projectile demo");
            return true;
        }
        Key::DemoPlotGrowth => {
            editor.replace_all(include_str!("../examples/plot_growth.py"));
            info!("Loaded plot growth demo");
            return true;
        }
        Key::DemoChartScatter => {
            editor.replace_all(include_str!("../examples/chart_scatter.py"));
            info!("Loaded chart scatter demo");
            return true;
        }
        Key::DemoChartHistogram => {
            editor.replace_all(include_str!("../examples/chart_histogram.py"));
            info!("Loaded chart histogram demo");
            return true;
        }
        Key::DemoChartBar => {
            editor.replace_all(include_str!("../examples/chart_bar.py"));
            info!("Loaded chart bar demo");
            return true;
        }
        Key::DemoChartPie => {
            editor.replace_all(include_str!("../examples/chart_pie.py"));
            info!("Loaded chart pie demo");
            return true;
        }
        Key::DemoNumericRc => {
            editor.replace_all(include_str!("../examples/numeric_rc.py"));
            info!("Loaded numeric RC demo");
            return true;
        }
        Key::DemoSnake => {
            editor.replace_all(include_str!("../examples/snake.py"));
            info!("Loaded snake demo");
            return true;
        }
        _ => {}
    }
    if key != Key::Run {
        editor.apply_with_page_height(key, page_lines);
        return true;
    }

    let result = execute(editor, right, live);
    draw_shortcuts(footer, result.free_bytes);
    if self_test && editor.text().starts_with("print('uart-ok')\n") {
        if result.status != 0 || !right.screen().contents().contains("uart-ok") {
            error!("UART editor execution test failed");
            return false;
        }
        info!("UART editor execution test passed");
    }
    if result.status < 0 {
        error!("MicroPython execution failed: {}", result.status);
        return false;
    } else if result.status > 0 && right.screen().contents().contains("KeyboardInterrupt") {
        info!("MicroPython execution interrupted");
    } else if result.status > 0 {
        info!("MicroPython script raised an exception");
    } else {
        info!("MicroPython execution completed");
    }
    true
}

fn process_key(
    key: Key,
    menu: &mut Menu,
    search: &mut Search,
    editor: &mut Editor,
    viewport: &mut Viewport,
    left: &mut vt100::Parser,
    right: &mut vt100::Parser,
    left_renderer: &mut TerminalRenderer,
    right_renderer: &mut TerminalRenderer,
    output_pane: &mut OutputPane,
    right_header: &mut Bar,
    footer: &mut Bar,
    pixels: &mut [u16],
    width: u16,
    height: u16,
    self_test: bool,
) -> bool {
    if key == Key::ToggleOutput {
        output_pane.toggle();
        return true;
    }
    if menu.state().is_none() && search.handle(key, editor) {
        return true;
    }
    let page = if menu.state().is_none()
        && matches!(
            key,
            Key::PageUp | Key::PageDown | Key::ShiftPageUp | Key::ShiftPageDown
        ) {
        Some((editor.text(), editor.cursor()))
    } else {
        None
    };
    let runs = (key == Key::Run && menu.state().is_none())
        || (key == Key::Enter && menu.state() == Some((2, 0)));
    if runs {
        draw_running(footer);
        viewport.render(
            editor,
            &Menu::default(),
            left,
            left_renderer.rows(),
            left_renderer.cols(),
        );
        let status = render_display(
            left,
            right,
            left_renderer,
            right_renderer,
            output_pane,
            right_header,
            footer,
            pixels,
            width,
            height,
        );
        if status != 0 {
            error!("Display update failed: {}", status);
            return false;
        }
        info!("MicroPython execution started");
    }
    let bar_height = footer.height();
    let mut live = LiveDisplay {
        renderer: right_renderer,
        pane: output_pane,
        pixels,
        width,
        height,
        bar_height,
        error: 0,
        self_test,
    };
    let result = apply_input(
        key,
        menu,
        editor,
        right,
        footer,
        Some(&mut live),
        self_test,
        usize::from(left_renderer.rows()).saturating_sub(1),
    );
    if let Some((text, old_cursor)) = page {
        viewport.scroll_page(&text, old_cursor, editor.cursor(), left_renderer.rows());
    }
    result
}

/// Initializes the application and runs the input and redraw loop.
///
/// Called by Zephyr through the C ABI. Initialization and runtime errors are
/// logged before returning; normal operation waits for UART input indefinitely.
#[no_mangle]
extern "C" fn rust_main() {
    unsafe { zephyr::set_logger().unwrap() };
    let self_test = unsafe { pocketgen_micropython_self_test() };
    if self_test < 0 {
        error!("MicroPython integration tests failed");
        return;
    }
    let mut width = 0;
    let mut height = 0;
    let result = unsafe { pocketgen_display_init(&mut width, &mut height) };
    if result != 0 {
        error!("Display initialization failed: {}", result);
        return;
    }
    if width < 2 || height == 0 {
        error!("Invalid display dimensions: {}x{}", width, height);
        return;
    }

    let right_x = width / 2;
    let left_width = right_x.saturating_sub(1);
    let right_width = width - right_x;
    let Some((probe, _)) = terminal(Size::new(left_width.into(), height.into())) else {
        error!("Display is too small for the terminal font");
        return;
    };
    let bar_height = probe.cell_size().1 as u16;
    if bar_height == 0 || u32::from(height) < u32::from(bar_height) * 3 {
        error!("Display is too short for the menu, output, and shortcut bars");
        return;
    }
    let left_size = Size::new(left_width.into(), (height - bar_height).into());
    let right_size = Size::new(right_width.into(), (height - 2 * bar_height).into());
    if self_test > 0 {
        #[cfg(target_arch = "riscv32")]
        {
            if !allocator::self_test() {
                error!("PSRAM allocator test failed");
                return;
            }
            info!("PSRAM allocator test passed");
        }
        if !test_terminal_bridge(width, height, right_x, right_size) {
            error!("Terminal bridge test failed");
            return;
        }
        info!("Terminal bridge test passed");
        if !test_editor_viewport(left_size) {
            error!("Editor viewport test failed");
            return;
        }
        info!("Editor viewport test passed");
        if !test_search(left_size) {
            error!("Search test failed");
            return;
        }
        info!("Search test passed");
        if !test_syntax_highlighting(left_size) {
            error!("Syntax highlighting test failed");
            return;
        }
        info!("Syntax highlighting test passed");
        if !test_selection_and_menu(left_size) {
            error!("Selection and menu test failed");
            return;
        }
        info!("Selection and menu test passed");
        if !test_demo_loading(right_size, Size::new(width.into(), bar_height.into())) {
            error!("Demo loading test failed");
            return;
        }
        info!("Demo loading test passed");
        if !test_demo_execution(
            right_size,
            include_str!("../examples/starfield.py"),
            "WARP // POCKETGEN",
        ) {
            error!("Starfield execution test failed");
            return;
        }
        info!("Starfield execution test passed");
        if !test_demo_execution(
            right_size,
            include_str!("../examples/cube.py"),
            "3D CUBE // POCKETGEN",
        ) {
            error!("Cube execution test failed");
            return;
        }
        info!("Cube execution test passed");
        if !test_demo_execution(
            right_size,
            include_str!("../examples/mexican_hat.py"),
            "MEXICAN HAT // POCKETGEN",
        ) {
            error!("Mexican hat execution test failed");
            return;
        }
        info!("Mexican hat execution test passed");
        if !test_terminal_controls(right_size) {
            error!("Terminal controls test failed");
            return;
        }
        info!("Terminal controls test passed");
        if !test_heap_status(right_size, Size::new(width.into(), bar_height.into())) {
            error!("MicroPython heap status test failed");
            return;
        }
        info!("MicroPython heap status test passed");
    }
    let Some((mut left_renderer, mut left)) = terminal(left_size) else {
        error!("Display is too small for the left terminal");
        return;
    };
    if left_renderer.rows() < 2 || left_renderer.cols() < 2 {
        error!("Display is too small for the editor and line numbers");
        return;
    }
    let Some((mut right_renderer, mut right)) = terminal(right_size) else {
        error!("Display is too small for the right terminal");
        return;
    };
    let mut output_pane = OutputPane::new(right_width, height - 2 * bar_height);
    let Some(mut right_header) =
        Bar::new(Size::new(right_width.into(), bar_height.into()), right_x, 0)
    else {
        error!("Display is too small for the output header");
        return;
    };
    let Some(mut footer) = Bar::new(
        Size::new(width.into(), bar_height.into()),
        0,
        height - bar_height,
    ) else {
        error!("Display is too small for the shortcut bar");
        return;
    };
    let version = unsafe { pocketgen_micropython_version() };
    if version.is_null() {
        error!("MicroPython version is unavailable");
        return;
    }
    let Ok(version) = unsafe { CStr::from_ptr(version) }.to_str() else {
        error!("MicroPython version is not UTF-8");
        return;
    };
    let header_cols = right_header.renderer.cols();
    blue_bar(
        &mut right_header.parser,
        header_cols,
        &format!(" MicroPython v{}", version),
    );
    draw_shortcuts(&mut footer, None);

    let autostart_cube = self_test == 0 && unsafe { pocketgen_autostart_cube_enabled() } != 0;
    let initial_source = if autostart_cube {
        include_str!("../examples/cube.py")
    } else {
        include_str!("../examples/terminal_demo.py")
    };
    let mut editor = Editor::new(initial_source);
    let mut menu = Menu::default();
    let mut search = Search::default();
    let mut viewport = Viewport { top: 0, left: 0 };
    highlight_benchmark::run(
        &editor,
        &mut viewport,
        &mut left,
        left_renderer.rows(),
        left_renderer.cols(),
    );
    viewport.render(
        &editor,
        &menu,
        &mut left,
        left_renderer.rows(),
        left_renderer.cols(),
    );
    let result = unsafe { pocketgen_input_init() };
    if result != 0 {
        error!("UART input initialization failed: {}", result);
        return;
    }
    let Some(mut pixels) = DisplayPixels::new(width as usize * height as usize) else {
        error!("Unable to allocate display buffer");
        return;
    };
    if self_test > 0 {
        if !test_graphics(
            &mut right_renderer,
            &mut right,
            &mut output_pane,
            &mut pixels,
            width,
            height,
            bar_height,
        ) {
            error!("Graphics integration test failed");
            return;
        }
        info!("Graphics integration test passed");
        output_pane.reset();
        right.process(b"\x1b[0m\x1b[2J\x1b[H\x1b[?25l");
    }
    let result = render_display(
        &left,
        &right,
        &mut left_renderer,
        &mut right_renderer,
        &output_pane,
        &mut right_header,
        &mut footer,
        &mut pixels,
        width,
        height,
    );
    if result != 0 {
        error!("Display update failed: {}", result);
        return;
    }
    if self_test > 0 {
        if !left
            .screen()
            .contents()
            .contains("from pocketgen import cls")
            || !right.screen().contents().trim().is_empty()
            || !footer.parser.screen().contents().contains("MP free: --")
        {
            error!("Startup editor display test failed");
            return;
        }
        info!("Startup editor display test passed");
        let Some(definitions) =
            include_str!("../examples/terminal_demo.py").strip_suffix("run_demo()\n")
        else {
            error!("Maze demo entry point is missing");
            return;
        };
        let test_editor = Editor::new(&format!("{}run_demo(8, 0)\n", definitions));
        let mut live = LiveDisplay {
            renderer: &mut right_renderer,
            pane: &mut output_pane,
            pixels: &mut pixels,
            width,
            height,
            bar_height,
            error: 0,
            self_test: true,
        };
        let result = execute(&test_editor, &mut right, Some(&mut live));
        draw_shortcuts(&mut footer, result.free_bytes);
        if result.status < 0 {
            error!("MicroPython execution failed: {}", result.status);
            return;
        } else if result.status > 0 && right.screen().contents().contains("KeyboardInterrupt") {
            info!("MicroPython execution interrupted");
        } else if result.status > 0 {
            info!("MicroPython script raised an exception");
        } else {
            info!("MicroPython execution completed");
        }
        let contents = right.screen().contents();
        let (rows, cols) = right.screen().size();
        let mut maze_width = (cols.get() - 1).min(39);
        let mut maze_height = (rows.get() - 2).min(21);
        if maze_width % 2 == 0 {
            maze_width -= 1;
        }
        if maze_height % 2 == 0 {
            maze_height -= 1;
        }
        let maze_left = (cols.get() - maze_width) / 2;
        if result.status != 0
            || !contents.contains("GOAL!")
            || !(0..maze_width).all(|x| {
                right.screen().cell(1, maze_left + x).is_some_and(|cell| {
                    cell.contents() == " " && cell.bgcolor() == vt100::Color::Idx(4)
                })
            })
            || !right
                .screen()
                .cell(2, maze_left + 1)
                .is_some_and(|cell| cell.bgcolor() == vt100::Color::Idx(0))
            || !right
                .screen()
                .cell(maze_height - 1, maze_left + maze_width - 2)
                .is_some_and(|cell| {
                    cell.contents() == "@"
                        && cell.fgcolor() == vt100::Color::Idx(10)
                        && cell.bgcolor() == vt100::Color::Idx(0)
                })
        {
            error!("Editor buffer execution test failed: {}", contents);
            return;
        }
        info!("Editor buffer execution test passed");
        let is_blue = |parser: &vt100::Parser, col: u16| {
            parser
                .screen()
                .cell(0, col)
                .is_some_and(|cell| cell.bgcolor() == vt100::Color::Idx(4))
        };
        let shortcuts = footer.parser.screen().contents();
        if !is_blue(&left, 0)
            || !is_blue(&left, left_renderer.cols() - 1)
            || !is_blue(&right_header.parser, right_header.renderer.cols() - 1)
            || !right_header
                .parser
                .screen()
                .contents()
                .contains(&format!("MicroPython v{}", version))
            || !shortcuts.contains("F10")
            || !shortcuts.contains("^R")
            || !shortcuts.contains("^G")
            || !shortcuts.contains("^Z")
            || !shortcuts.contains("^V")
            || !shortcuts.ends_with(&format!("MP free: {} B", result.free_bytes.unwrap_or(0)))
            || !is_blue(&footer.parser, footer.renderer.cols() - 1)
        {
            error!("Menu and shortcut bar test failed");
            return;
        }
        let mut check_pixels = vec![0x4321; width as usize * height as usize];
        footer.render(&mut check_pixels, width);
        let footer_start = footer.y as usize * width as usize;
        if check_pixels[..footer_start]
            .iter()
            .any(|pixel| *pixel != 0x4321)
            || check_pixels[footer_start..]
                .iter()
                .any(|pixel| *pixel == 0x4321)
        {
            error!("Shortcut bar is not confined to the bottom of the display");
            return;
        }
        info!("Menu and shortcut bar test passed");
    }

    let result = render_display(
        &left,
        &right,
        &mut left_renderer,
        &mut right_renderer,
        &output_pane,
        &mut right_header,
        &mut footer,
        &mut pixels,
        width,
        height,
    );
    if result != 0 {
        error!("Display update failed: {}", result);
        return;
    }
    if self_test > 0 {
        let (_, cols) = right.screen().size();
        let mut maze_width = (cols.get() - 1).min(39);
        if maze_width % 2 == 0 {
            maze_width -= 1;
        }
        let maze_left = (cols.get() - maze_width) / 2;
        let (cell_width, cell_height) = right_renderer.cell_size();
        let wall_x = right_x as usize + maze_left as usize * cell_width as usize;
        let wall_y = bar_height as usize + cell_height as usize;
        let wall_color = RawU16::from(Rgb565::new(0, 0, 16)).into_inner();
        if !(wall_y..wall_y + cell_height as usize).all(|y| {
            (wall_x..wall_x + cell_width as usize)
                .all(|x| pixels[y * width as usize + x] == wall_color)
        }) {
            error!("Maze wall is not a solid blue cell");
            return;
        }
        info!("Solid blue maze wall test passed");
        let divider_color = RawU16::from(Rgb565::new(12, 24, 12)).into_inner();
        if !(bar_height..height - bar_height)
            .all(|y| pixels[y as usize * width as usize + left_width as usize] == divider_color)
            || pixels[(height - bar_height) as usize * width as usize + left_width as usize]
                == divider_color
        {
            error!("Pane divider test failed");
            return;
        }
        info!("Pane divider test passed");
    }
    info!("Rendered two terminals on a {}x{} display", width, height);

    info!("Editor input ready on virtual UART");
    if autostart_cube {
        info!("Autostarting 3D Cube demo");
        if !process_key(
            Key::Run,
            &mut menu,
            &mut search,
            &mut editor,
            &mut viewport,
            &mut left,
            &mut right,
            &mut left_renderer,
            &mut right_renderer,
            &mut output_pane,
            &mut right_header,
            &mut footer,
            &mut pixels,
            width,
            height,
            false,
        ) {
            return;
        }
        viewport.render(
            &editor,
            &menu,
            &mut left,
            left_renderer.rows(),
            left_renderer.cols(),
        );
        let result = render_display(
            &left,
            &right,
            &mut left_renderer,
            &mut right_renderer,
            &output_pane,
            &mut right_header,
            &mut footer,
            &mut pixels,
            width,
            height,
        );
        if result != 0 {
            error!("Display update failed: {}", result);
            return;
        }
    }
    let mut decoder = Decoder::default();
    let mut idle = 0;
    loop {
        let mut changed = false;
        let mut received = false;
        for _ in 0..256 {
            let mut byte = 0;
            let result = unsafe { pocketgen_input_poll(&mut byte) };
            if result < 0 {
                error!("UART input failed: {}", result);
                return;
            }
            if result == 0 {
                break;
            }
            received = true;
            if let Some(key) = decoder.escape_before(byte) {
                if !process_key(
                    key,
                    &mut menu,
                    &mut search,
                    &mut editor,
                    &mut viewport,
                    &mut left,
                    &mut right,
                    &mut left_renderer,
                    &mut right_renderer,
                    &mut output_pane,
                    &mut right_header,
                    &mut footer,
                    &mut pixels,
                    width,
                    height,
                    self_test > 0,
                ) {
                    return;
                }
                changed = true;
            }
            match decoder.feed(byte) {
                Ok(Some(key)) => {
                    if !process_key(
                        key,
                        &mut menu,
                        &mut search,
                        &mut editor,
                        &mut viewport,
                        &mut left,
                        &mut right,
                        &mut left_renderer,
                        &mut right_renderer,
                        &mut output_pane,
                        &mut right_header,
                        &mut footer,
                        &mut pixels,
                        width,
                        height,
                        self_test > 0,
                    ) {
                        return;
                    }
                    changed = true;
                }
                Ok(None) => {}
                Err(err) => warn!("Invalid input: {:?}", err),
            }
        }
        if received {
            idle = 0;
        } else if decoder.pending() {
            idle += 1;
            let limit = if decoder.pending_escape() { 5 } else { 50 };
            if idle >= limit {
                match decoder.timeout() {
                    Ok(Some(key)) => {
                        if !process_key(
                            key,
                            &mut menu,
                            &mut search,
                            &mut editor,
                            &mut viewport,
                            &mut left,
                            &mut right,
                            &mut left_renderer,
                            &mut right_renderer,
                            &mut output_pane,
                            &mut right_header,
                            &mut footer,
                            &mut pixels,
                            width,
                            height,
                            self_test > 0,
                        ) {
                            return;
                        }
                        changed = true;
                    }
                    Ok(None) => {}
                    Err(err) => warn!("Incomplete input sequence: {:?}", err),
                }
                idle = 0;
            }
        }
        if changed {
            viewport.render(
                &editor,
                &menu,
                &mut left,
                left_renderer.rows(),
                left_renderer.cols(),
            );
            viewport.render_search(&search, &mut left, left_renderer.cols());
            let result = render_display(
                &left,
                &right,
                &mut left_renderer,
                &mut right_renderer,
                &output_pane,
                &mut right_header,
                &mut footer,
                &mut pixels,
                width,
                height,
            );
            if result != 0 {
                error!("Display update failed: {}", result);
                return;
            }
        }
        unsafe { pocketgen_input_wait() };
    }
}
