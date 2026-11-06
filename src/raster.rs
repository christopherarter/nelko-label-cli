use std::{fs::File, io::BufWriter, path::Path};

use anyhow::{Context, Result, ensure};

use crate::profile::Media;

#[derive(Debug)]
pub(crate) struct Label {
    /// Monochrome raster, most significant pixel first, 1 = white.
    bitmap: Vec<u8>,
    copies: u16,
    media: Media,
}

impl Label {
    pub(crate) fn from_gray(
        gray: Vec<u8>,
        media: Media,
        threshold: u8,
        copies: u16,
    ) -> Result<Self> {
        let (width, height) = media.dimensions();
        ensure!(
            width > 0 && height > 0 && width <= 8192 && height <= 8192,
            "unsupported raster dimensions"
        );
        ensure!(gray.len() == width * height, "invalid raster dimensions");
        ensure!(
            (1..=100).contains(&copies),
            "copies must be between 1 and 100"
        );
        ensure!(threshold > 0, "threshold must be greater than zero");
        let row_bytes = width.div_ceil(8);
        // Padding bits are white so a partial final byte cannot create a black stripe.
        let mut bitmap = vec![255; row_bytes * height];
        for (index, pixel) in gray.into_iter().enumerate() {
            if pixel < threshold {
                let (x, y) = (index % width, index / width);
                bitmap[y * row_bytes + x / 8] &= !(0x80 >> (x % 8));
            }
        }
        Ok(Self {
            bitmap,
            copies,
            media,
        })
    }

    pub(crate) fn media(&self) -> Media {
        self.media
    }
    pub(crate) fn copies(&self) -> u16 {
        self.copies
    }
    pub(crate) fn bitmap(&self) -> &[u8] {
        &self.bitmap
    }
    pub(crate) fn row_bytes(&self) -> usize {
        self.media.dimensions().0.div_ceil(8)
    }

    pub(crate) fn save_png(&self, path: &Path) -> Result<()> {
        let (width, height) = self.media.dimensions();
        let mut gray = vec![255; width * height];
        let row_bytes = width.div_ceil(8);
        for (index, pixel) in gray.iter_mut().enumerate() {
            let (x, y) = (index % width, index / width);
            if self.bitmap[y * row_bytes + x / 8] & (0x80 >> (x % 8)) == 0 {
                *pixel = 0;
            }
        }
        let writer = BufWriter::new(
            File::create(path).with_context(|| format!("creating {}", path.display()))?,
        );
        let mut encoder = png::Encoder::new(writer, width as u32, height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&gray)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::NELKO;

    #[test]
    fn packs_pixels_in_row_order_with_white_padding_and_exact_print_count() -> Result<()> {
        let media = NELKO.media;
        let (width, height) = media.dimensions();
        let mut gray = vec![255; width * height];
        gray[0] = 0;
        gray[7] = 0;
        gray[8] = 159;
        gray[width - 1] = 0;
        gray[width] = 160;
        let label = Label::from_gray(gray, media, 160, 2)?;
        assert_eq!(label.bitmap[0], 0x7e);
        assert_eq!(label.bitmap[1], 0x7f);
        assert_eq!(label.bitmap[width.div_ceil(8) - 1], 0xef);
        assert_eq!(label.bitmap[width.div_ceil(8)], 255);
        let job = crate::protocol::encode(NELKO.language, &label);
        assert!(job.ends_with(b"\r\nPRINT 1,2\r\n"));
        assert!(job.starts_with(b"SIZE 4,6\r\n"));
        Ok(())
    }

    #[test]
    fn rejects_invalid_jobs() {
        assert!(Label::from_gray(vec![0], NELKO.media, 160, 1).is_err());
        assert!(Label::from_gray(vec![255; 812 * 1218], NELKO.media, 160, 0).is_err());
    }
}
