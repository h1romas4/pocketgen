use log::info;
use pocketgen_editor::{Editor, Menu};
use pocketgen_terminal::vt100;

use crate::viewport::Viewport;

extern "C" {
    fn pocketgen_highlight_benchmark_enabled() -> i32;
    fn pocketgen_benchmark_ticks() -> u32;
    fn pocketgen_benchmark_ticks_to_us(ticks: u64) -> u64;
}

pub(crate) fn run(
    editor: &Editor,
    viewport: &mut Viewport,
    parser: &mut vt100::Parser,
    rows: u16,
    cols: u16,
) {
    if unsafe { pocketgen_highlight_benchmark_enabled() } == 0 {
        return;
    }

    const SAMPLES: u64 = 40;
    let menu = Menu::default();
    for enabled in [false, true] {
        viewport.render_with_highlight(editor, &menu, parser, rows, cols, enabled);
    }
    let mut cycles = [0u64; 2];
    for sample in 0..SAMPLES {
        for enabled in [sample % 2 != 0, sample % 2 == 0] {
            let start = unsafe { pocketgen_benchmark_ticks() };
            viewport.render_with_highlight(editor, &menu, parser, rows, cols, enabled);
            cycles[usize::from(enabled)] +=
                u64::from(unsafe { pocketgen_benchmark_ticks() }.wrapping_sub(start));
        }
    }
    let plain_us = unsafe { pocketgen_benchmark_ticks_to_us(cycles[0]) } / SAMPLES;
    let color_us = unsafe { pocketgen_benchmark_ticks_to_us(cycles[1]) } / SAMPLES;
    info!(
        "Highlight benchmark: {} viewport renders, {}x{}, plain={} us/frame, color={} us/frame (display excluded)",
        SAMPLES, cols, rows, plain_us, color_us
    );
}
