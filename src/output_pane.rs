//! Retained graphics and text presentation for the Python output pane.

use embedded_graphics::geometry::Size;
use log::{info, warn};
use pocketgen_terminal::{vt100, Rgb565Surface, TerminalRenderer};

use crate::display_buffer::DisplayPixels;

const EINVAL: i32 = -22;
const ENOMEM: i32 = -12;
const ENODEV: i32 = -19;

pub(crate) struct OutputPane {
    width: u16,
    height: u16,
    graphics: Option<DisplayPixels>,
    showing_graphics: bool,
    manual_view: bool,
}

impl OutputPane {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            graphics: None,
            showing_graphics: false,
            manual_view: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.graphics = None;
        self.showing_graphics = false;
        self.manual_view = false;
    }

    pub(crate) fn show_text(&mut self) {
        if !self.manual_view {
            self.showing_graphics = false;
        }
    }

    pub(crate) fn show_error(&mut self) {
        if self.showing_graphics {
            info!("Output view: text (Python exception)");
        }
        self.showing_graphics = false;
    }

    pub(crate) fn toggle(&mut self) -> bool {
        if self.graphics.is_none() {
            warn!("Cannot switch output view: no graphics available");
            return false;
        }
        self.showing_graphics = !self.showing_graphics;
        self.manual_view = true;
        info!(
            "Output view: {}",
            if self.showing_graphics {
                "graphics"
            } else {
                "text"
            }
        );
        true
    }

    pub(crate) fn graphics_command(
        &mut self,
        operation: u32,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u32,
    ) -> i32 {
        match operation {
            0 => return i32::from(self.width),
            1 => return i32::from(self.height),
            2 if color <= u16::MAX.into() => {
                if self.graphics.is_none() {
                    let Some(pixels) =
                        DisplayPixels::new(usize::from(self.width) * usize::from(self.height))
                    else {
                        return ENOMEM;
                    };
                    self.graphics = Some(pixels);
                }
                self.graphics.as_mut().unwrap().fill(color as u16);
            }
            3 | 4 if color <= u16::MAX.into() => {
                if x0 < 0
                    || y0 < 0
                    || x0 >= i32::from(self.width)
                    || y0 >= i32::from(self.height)
                    || (operation == 4
                        && (x1 < 0
                            || y1 < 0
                            || x1 >= i32::from(self.width)
                            || y1 >= i32::from(self.height)))
                {
                    return EINVAL;
                }
                let Some(pixels) = self.graphics.as_mut() else {
                    return ENODEV;
                };
                let (x1, y1) = if operation == 3 { (x0, y0) } else { (x1, y1) };
                let (mut x, mut y) = (x0, y0);
                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let sx = if x0 < x1 { 1 } else { -1 };
                let sy = if y0 < y1 { 1 } else { -1 };
                let mut error = dx - dy;
                loop {
                    pixels[y as usize * usize::from(self.width) + x as usize] = color as u16;
                    if x == x1 && y == y1 {
                        break;
                    }
                    let twice = 2 * error;
                    if twice > -dy {
                        error -= dy;
                        x += sx;
                    }
                    if twice < dx {
                        error += dx;
                        y += sy;
                    }
                }
            }
            5 if self.graphics.is_some() => {}
            5 => return ENODEV,
            _ => return EINVAL,
        }
        if !self.manual_view {
            self.showing_graphics = true;
        }
        0
    }

    pub(crate) fn render(
        &self,
        parser: &vt100::Parser,
        renderer: &mut TerminalRenderer,
        pixels: &mut [u16],
        display_width: u16,
        x: u16,
        y: u16,
    ) {
        if self.showing_graphics {
            let graphics = self.graphics.as_ref().expect("visible graphics must exist");
            let width = usize::from(self.width);
            for row in 0..usize::from(self.height) {
                let destination =
                    (usize::from(y) + row) * usize::from(display_width) + usize::from(x);
                pixels[destination..destination + width]
                    .copy_from_slice(&graphics[row * width..(row + 1) * width]);
            }
        } else {
            renderer
                .render(
                    parser.screen(),
                    &mut Rgb565Surface::new(
                        pixels,
                        display_width,
                        Size::new(self.width.into(), self.height.into()),
                        x,
                        y,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
    }
}
