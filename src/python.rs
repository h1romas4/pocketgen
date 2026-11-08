//! Synchronous MicroPython execution and terminal output bridging.

use core::ffi::c_void;
use log::{info, warn};
use pocketgen_editor::{Decoder, Editor, Key};
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
        key: Option<extern "C" fn(*mut c_void, *mut u8, usize) -> i32>,
        context: *mut c_void,
        free_bytes: *mut usize,
    ) -> i32;
    fn pocketgen_input_ticks_ms() -> u32;
}

pub(crate) struct LiveDisplay<'a> {
    pub(crate) renderer: &'a mut TerminalRenderer,
    pub(crate) pane: &'a mut OutputPane,
    pub(crate) pixels: &'a mut [u16],
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) bar_height: u16,
    pub(crate) error: i32,
    pub(crate) self_test: bool,
}

struct KeyInput {
    decoder: Decoder,
    queue: [Option<Key>; 64],
    head: usize,
    len: usize,
    last_byte: u32,
    enabled: bool,
    overflow: bool,
    ignored: bool,
}

impl Default for KeyInput {
    fn default() -> Self {
        Self {
            decoder: Decoder::default(),
            queue: [None; 64],
            head: 0,
            len: 0,
            last_byte: 0,
            enabled: false,
            overflow: false,
            ignored: false,
        }
    }
}

impl KeyInput {
    fn push(&mut self, key: Key) {
        if self.len == self.queue.len() {
            if !self.overflow {
                warn!("MicroPython key input queue overflow");
                self.overflow = true;
            }
        } else {
            self.queue[(self.head + self.len) % self.queue.len()] = Some(key);
            self.len += 1;
        }
    }

    fn accept(&mut self, result: Result<Option<Key>, pocketgen_editor::InputError>) {
        match result {
            Ok(Some(key)) => self.push(key),
            Ok(None) => {}
            Err(error) => warn!("Invalid input: {:?}", error),
        }
    }

    fn feed(&mut self, byte: u8, now: u32) {
        if let Some(key) = self.decoder.escape_before(byte) {
            self.push(key);
        }
        let result = self.decoder.feed(byte);
        if matches!(result, Ok(None)) && !self.decoder.pending() && byte < 0x20 && byte != b'\n' {
            warn!("Unsupported MicroPython control byte: {:#04x}", byte);
        }
        self.accept(result);
        self.last_byte = now;
    }

    fn expire(&mut self, now: u32) {
        let timeout = if self.decoder.pending_escape() { 50 } else { 500 };
        if self.decoder.pending() && now.wrapping_sub(self.last_byte) >= timeout {
            let result = self.decoder.timeout();
            self.accept(result);
        }
    }

    fn pop(&mut self) -> Option<Key> {
        if self.len == 0 {
            return None;
        }
        let key = self.queue[self.head].take();
        self.head = (self.head + 1) % self.queue.len();
        self.len -= 1;
        key
    }
}

struct PythonOutput<'a, 'b, 'c> {
    parser: &'a mut vt100::Parser,
    live: Option<&'b mut LiveDisplay<'c>>,
    dirty: bool,
    keys: KeyInput,
}

extern "C" fn python_key(context: *mut c_void, buffer: *mut u8, capacity: usize) -> i32 {
    let output = unsafe { &mut *context.cast::<PythonOutput<'_, '_, '_>>() };
    output.keys.enabled = true;
    if output.keys.overflow {
        output.keys.overflow = false;
        return -105; // ENOBUFS
    }
    while let Some(key) = output.keys.pop() {
        let mut encoded = [0; 4];
        let value = match key {
            Key::Character('\t') => "TAB",
            Key::Character(ch) => ch.encode_utf8(&mut encoded),
            Key::Up => "UP",
            Key::Down => "DOWN",
            Key::Left => "LEFT",
            Key::Right => "RIGHT",
            Key::Enter => "ENTER",
            Key::Escape => "ESC",
            Key::Backspace => "BACKSPACE",
            Key::Delete => "DELETE",
            Key::Home => "HOME",
            Key::End => "END",
            Key::PageUp => "PAGEUP",
            Key::PageDown => "PAGEDOWN",
            _ => {
                warn!("Unsupported MicroPython key: {:?}", key);
                continue;
            }
        };
        if value.len() > capacity {
            return -22;
        }
        let target = unsafe { core::slice::from_raw_parts_mut(buffer, value.len()) };
        target.copy_from_slice(value.as_bytes());
        return value.len() as i32;
    }
    0
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
        if output.keys.enabled {
            output.keys.feed(byte, unsafe { pocketgen_input_ticks_ms() });
        } else if !output.keys.ignored {
            warn!("Input other than Ctrl+C and Ctrl+G is ignored while MicroPython runs");
            output.keys.ignored = true;
        }
    }
    if output.keys.enabled {
        output.keys.expire(unsafe { pocketgen_input_ticks_ms() });
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
        keys: KeyInput::default(),
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
            progress.map(|_| python_key as extern "C" fn(*mut c_void, *mut u8, usize) -> i32),
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
