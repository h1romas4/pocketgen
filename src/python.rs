//! Synchronous MicroPython execution and terminal output bridging.

use core::ffi::c_void;
use log::{info, warn};
use pocketgen_editor::Editor;
use pocketgen_terminal::{process_output, vt100, TerminalRenderer};

use crate::output_pane::OutputPane;
use crate::ui::pocketgen_display_present;

extern "C" {
    fn pocketgen_micropython_run(
        source: *const u8,
        len: usize,
        cols: u16,
        rows: u16,
        callback: extern "C" fn(*mut c_void, *const u8, usize),
        progress: Option<extern "C" fn(*mut c_void, bool) -> i32>,
        graphics: Option<extern "C" fn(*mut c_void, u32, i32, i32, i32, i32, u32) -> i32>,
        context: *mut c_void,
        free_bytes: *mut usize,
    ) -> i32;
}

pub(crate) struct LiveDisplay<'a> {
    pub(crate) renderer: &'a mut TerminalRenderer,
    pub(crate) pane: &'a mut OutputPane,
    pub(crate) pixels: &'a mut [u16],
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) bar_height: u16,
    pub(crate) error: i32,
    pub(crate) ignored_input: bool,
    pub(crate) self_test: bool,
}

struct PythonOutput<'a, 'b, 'c> {
    parser: &'a mut vt100::Parser,
    live: Option<&'b mut LiveDisplay<'c>>,
    dirty: bool,
}

extern "C" fn python_output(context: *mut c_void, bytes: *const u8, len: usize) {
    if len == 0 {
        return;
    }
    // Both callbacks are synchronous and use the context borrowed by execute.
    let output = unsafe { &mut *context.cast::<PythonOutput<'_, '_, '_>>() };
    let bytes = unsafe { core::slice::from_raw_parts(bytes, len) };
    process_output(output.parser, bytes);
    if let Some(live) = output.live.as_mut() {
        live.pane.show_text();
    }
    output.dirty = true;
}

extern "C" fn python_graphics(
    context: *mut c_void,
    operation: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: u32,
) -> i32 {
    let output = unsafe { &mut *context.cast::<PythonOutput<'_, '_, '_>>() };
    let Some(live) = output.live.as_mut() else {
        return -19;
    };
    let result = live.pane.graphics_command(operation, x0, y0, x1, y1, color);
    if result == 0 && operation >= 2 {
        output.dirty = true;
    }
    result
}

extern "C" fn python_progress(context: *mut c_void, refresh: bool) -> i32 {
    let output = unsafe { &mut *context.cast::<PythonOutput<'_, '_, '_>>() };
    let Some(live) = output.live.as_mut() else {
        return 0;
    };
    if live.error != 0 {
        return live.error;
    }
    for _ in 0..32 {
        let mut byte = 0;
        let status = unsafe { crate::pocketgen_input_poll(&mut byte) };
        if status < 0 {
            live.error = status;
            return status;
        }
        if status == 0 {
            break;
        }
        if byte == 3 {
            return 1;
        }
        if byte == 7 {
            if live.pane.toggle() {
                output.dirty = true;
            }
            continue;
        }
        if !live.ignored_input {
            warn!("Input other than Ctrl+C and Ctrl+G is ignored while MicroPython runs");
            live.ignored_input = true;
        }
    }
    if refresh && output.dirty {
        let right_x = live.width / 2;
        live.pane.render(
            output.parser,
            live.renderer,
            live.pixels,
            live.width,
            right_x,
            live.bar_height,
        );
        let status =
            unsafe { pocketgen_display_present(live.pixels.as_ptr(), live.width, live.height) };
        if status != 0 {
            live.error = status;
            return status;
        }
        output.dirty = false;
        if live.self_test {
            info!("Live output frame presented");
        }
    }
    0
}

/// Result of executing an editor buffer in MicroPython.
pub(crate) struct Execution {
    pub(crate) status: i32,
    pub(crate) free_bytes: Option<usize>,
}

/// Clears the output pane and executes a snapshot of the editor document.
pub(crate) fn execute(
    editor: &Editor,
    right: &mut vt100::Parser,
    mut live: Option<&mut LiveDisplay<'_>>,
) -> Execution {
    right.process(b"\x1b[0m\x1b[2J\x1b[H\x1b[?25l");
    if let Some(live) = live.as_mut() {
        live.pane.reset();
    }
    let source = editor.text();
    let (rows, cols) = right.screen().size();
    let mut free_bytes = 0;
    let mut output = PythonOutput {
        parser: right,
        live,
        dirty: true,
    };
    let progress = output
        .live
        .is_some()
        .then_some(python_progress as extern "C" fn(*mut c_void, bool) -> i32);
    let status = unsafe {
        pocketgen_micropython_run(
            source.as_ptr(),
            source.len(),
            cols.get(),
            rows.get(),
            python_output,
            progress,
            Some(python_graphics),
            (&mut output as *mut PythonOutput<'_, '_, '_>).cast(),
            &mut free_bytes,
        )
    };
    let display_error = output.live.as_ref().map_or(0, |live| live.error);
    if status > 0 {
        if let Some(live) = output.live.as_mut() {
            live.pane.show_error();
        }
    }
    Execution {
        status: if display_error != 0 {
            display_error
        } else {
            status
        },
        free_bytes: (status >= 0 && display_error == 0).then_some(free_bytes),
    }
}
