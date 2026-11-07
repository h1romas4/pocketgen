use core::convert::Infallible;
use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::{Rgb565, raw::RawU16};
use embedded_graphics::prelude::RawData;

pub struct Rgb565Surface<'a> {
    pixels: &'a mut [u16],
    width: u16,
    size: Size,
    x_offset: u16,
    y_offset: u16,
}

impl<'a> Rgb565Surface<'a> {
    pub fn new(
        pixels: &'a mut [u16],
        width: u16,
        size: Size,
        x_offset: u16,
        y_offset: u16,
    ) -> Option<Self> {
        let row_width = usize::from(width);
        let x = usize::from(x_offset);
        let surface_width = usize::try_from(size.width).ok()?;
        let surface_height = usize::try_from(size.height).ok()?;
        let row_end = x.checked_add(surface_width)?;
        if row_end > row_width {
            return None;
        }
        if surface_height > 0 {
            let last_row = usize::from(y_offset).checked_add(surface_height - 1)?;
            let end = last_row.checked_mul(row_width)?.checked_add(row_end)?;
            if end > pixels.len() {
                return None;
            }
        }
        Some(Self {
            pixels,
            width,
            size,
            x_offset,
            y_offset,
        })
    }
}

impl OriginDimensions for Rgb565Surface<'_> {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Rgb565Surface<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0
                && point.y >= 0
                && (point.x as u32) < self.size.width
                && (point.y as u32) < self.size.height
            {
                let index = (point.y as usize + usize::from(self.y_offset))
                    * usize::from(self.width)
                    + usize::from(self.x_offset)
                    + point.x as usize;
                self.pixels[index] = RawU16::from(color).into_inner();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::geometry::Point;
    use embedded_graphics::pixelcolor::RgbColor;

    #[test]
    fn clips_to_offset_surface() {
        let mut pixels = [0u16; 20];
        let mut surface = Rgb565Surface::new(&mut pixels, 5, Size::new(2, 2), 2, 1).unwrap();
        surface
            .draw_iter([
                Pixel(Point::new(0, 0), Rgb565::RED),
                Pixel(Point::new(1, 1), Rgb565::WHITE),
                Pixel(Point::new(-1, 0), Rgb565::WHITE),
                Pixel(Point::new(2, 0), Rgb565::WHITE),
                Pixel(Point::new(0, 2), Rgb565::WHITE),
            ])
            .unwrap();
        assert_eq!(pixels[7], RawU16::from(Rgb565::RED).into_inner());
        assert_eq!(pixels[13], RawU16::from(Rgb565::WHITE).into_inner());
        assert_eq!(pixels.iter().filter(|&&pixel| pixel != 0).count(), 2);
    }

    #[test]
    fn rejects_out_of_bounds_surface() {
        let mut pixels = [0u16; 20];
        assert!(Rgb565Surface::new(&mut pixels, 5, Size::new(3, 1), 3, 0).is_none());
        assert!(Rgb565Surface::new(&mut pixels, 5, Size::new(2, 2), 0, 3).is_none());
        assert!(Rgb565Surface::new(&mut pixels, 5, Size::new(2, u32::MAX), 0, 1).is_none());
    }
}
