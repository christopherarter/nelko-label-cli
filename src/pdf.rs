use std::path::Path;

use crate::profile::Media;
use anyhow::Result;

#[cfg(all(target_os = "macos", not(feature = "portable-pdf")))]
pub(crate) fn render(path: &Path, page_number: Option<u32>, media: Media) -> Result<Vec<u8>> {
    use anyhow::{Context, ensure};
    use objc2_core_foundation::{CFURL, CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGPDFBox, CGPDFDocument,
        CGPDFPage,
    };

    let (pixel_width, pixel_height) = media.dimensions();
    ensure!(
        pixel_width > 0 && pixel_height > 0 && pixel_width <= 8192 && pixel_height <= 8192,
        "unsupported profile raster dimensions"
    );

    let url = CFURL::from_file_path(path).context("creating PDF file URL")?;
    let document = CGPDFDocument::with_url(Some(&url))
        .with_context(|| format!("opening PDF {}", path.display()))?;
    ensure!(
        CGPDFDocument::is_unlocked(Some(&document)),
        "PDF is password protected"
    );
    let pages = CGPDFDocument::number_of_pages(Some(&document));
    ensure!(pages > 0, "PDF contains no pages");
    ensure!(
        pages == 1 || page_number.is_some(),
        "PDF contains {pages} pages; select one with --page N"
    );
    let number = page_number.unwrap_or(1) as usize;
    ensure!(
        number <= pages,
        "page {number} does not exist (PDF has {pages} pages)"
    );
    let page = CGPDFDocument::page(Some(&document), number).context("loading PDF page")?;
    let bounds = CGPDFPage::box_rect(Some(&page), CGPDFBox::CropBox);
    ensure!(
        bounds.size.width.is_finite()
            && bounds.size.height.is_finite()
            && bounds.size.width > 0.0
            && bounds.size.height > 0.0,
        "PDF page has invalid dimensions"
    );
    let rotation = CGPDFPage::rotation_angle(Some(&page));
    let (width, height) = if rotation.rem_euclid(180) == 90 {
        (bounds.size.height, bounds.size.width)
    } else {
        (bounds.size.width, bounds.size.height)
    };
    let portrait = (width / height - media.width_inches / media.height_inches).abs();
    let landscape = (width / height - media.height_inches / media.width_inches).abs();
    ensure!(
        portrait < 0.025 || landscape < 0.025,
        "PDF page is {:.1}×{:.1} inches, not the selected label aspect ratio; crop or re-export it first",
        width / 72.0,
        height / 72.0
    );
    let space = CGColorSpace::new_device_gray().context("creating grayscale color space")?;
    let mut pixels = vec![255_u8; pixel_width * pixel_height];
    // SAFETY: pixels owns pixel_width*pixel_height writable bytes with one byte per gray pixel.
    // It is never reallocated or accessed while the context can write to it. The
    // context is dropped before pixels is returned, including on error paths.
    let context = unsafe {
        CGBitmapContextCreate(
            pixels.as_mut_ptr().cast(),
            pixel_width,
            pixel_height,
            8,
            pixel_width,
            Some(&space),
            CGImageAlphaInfo::None.0,
        )
    }
    .context("creating PDF raster context")?;
    let extra_rotation = if landscape < portrait { 90 } else { 0 };
    let (out_width, out_height) = if extra_rotation == 90 {
        (height, width)
    } else {
        (width, height)
    };
    // CGPDFPageGetDrawingTransform centers small pages without upscaling them.
    // Apply the points-to-dots scale explicitly, then let Quartz handle rotation
    // and nonzero page origins in the page's natural coordinate system.
    let scale = (pixel_width as f64 / out_width).min(pixel_height as f64 / out_height);
    CGContext::translate_ctm(
        Some(&context),
        (pixel_width as f64 - out_width * scale) / 2.0,
        (pixel_height as f64 - out_height * scale) / 2.0,
    );
    CGContext::scale_ctm(Some(&context), scale, scale);
    let target = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: out_width,
            height: out_height,
        },
    };
    let transform =
        CGPDFPage::drawing_transform(Some(&page), CGPDFBox::CropBox, target, extra_rotation, true);
    CGContext::concat_ctm(Some(&context), transform);
    CGContext::set_should_antialias(Some(&context), false);
    CGContext::set_should_smooth_fonts(Some(&context), false);
    CGContext::draw_pdf_page(Some(&context), Some(&page));
    CGContext::flush(Some(&context));
    drop(context);
    Ok(pixels)
}

#[cfg(any(target_os = "linux", feature = "portable-pdf"))]
pub(crate) fn render(path: &Path, page_number: Option<u32>, media: Media) -> Result<Vec<u8>> {
    use anyhow::{Context, ensure};
    use hayro::{
        PixmapSettings, RenderCache, RenderSettings, hayro_interpret::InterpreterSettings,
        hayro_syntax::Pdf,
    };

    let (pixel_width, pixel_height) = media.dimensions();
    ensure!(
        pixel_width > 0 && pixel_height > 0 && pixel_width <= 8192 && pixel_height <= 8192,
        "unsupported profile raster dimensions"
    );
    let bytes = std::fs::read(path).with_context(|| format!("reading PDF {}", path.display()))?;
    let pdf = Pdf::new(bytes).map_err(|error| {
        anyhow::anyhow!(
            "opening PDF {}: {error:?} (password-protected PDFs are unsupported)",
            path.display()
        )
    })?;
    let pages = pdf.pages();
    ensure!(!pages.is_empty(), "PDF contains no pages");
    ensure!(
        pages.len() == 1 || page_number.is_some(),
        "PDF contains {} pages; select one with --page N",
        pages.len()
    );
    let index = page_number
        .unwrap_or(1)
        .checked_sub(1)
        .context("page numbers start at one")? as usize;
    let page = pages.get(index).with_context(|| {
        format!(
            "page {} does not exist (PDF has {} pages)",
            index + 1,
            pages.len()
        )
    })?;
    let (width, height) = page.render_dimensions();
    ensure!(
        width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
        "PDF page has invalid dimensions"
    );
    let portrait = (f64::from(width / height) - media.width_inches / media.height_inches).abs();
    let landscape = (f64::from(width / height) - media.height_inches / media.width_inches).abs();
    ensure!(
        portrait < 0.025 || landscape < 0.025,
        "PDF page is {:.1}×{:.1} inches, not the selected label aspect ratio; crop or re-export it first",
        width / 72.0,
        height / 72.0
    );
    let rotate = landscape < portrait;
    let (target_width, target_height) = if rotate {
        (pixel_height, pixel_width)
    } else {
        (pixel_width, pixel_height)
    };
    let scale = (target_width as f32 / width).min(target_height as f32 / height);
    let pixmap = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        },
    );
    let render_width = usize::from(pixmap.width());
    let render_height = usize::from(pixmap.height());
    ensure!(render_width > 0 && render_height > 0, "PDF raster is empty");
    let rgba = pixmap.take_rgba8(hayro::vello_cpu::peniko::ImageAlphaType::Alpha);
    let (out_width, out_height) = if rotate {
        (render_height, render_width)
    } else {
        (render_width, render_height)
    };
    let pad_x = pixel_width.saturating_sub(out_width) / 2;
    let pad_y = pixel_height.saturating_sub(out_height) / 2;
    let mut pixels = vec![255; pixel_width * pixel_height];
    for y in 0..render_height {
        for x in 0..render_width {
            let (target_x, target_y) = if rotate {
                (render_height - 1 - y + pad_x, x + pad_y)
            } else {
                (x + pad_x, y + pad_y)
            };
            if target_x < pixel_width && target_y < pixel_height {
                let offset = (y * render_width + x) * 4;
                let color = &rgba[offset..offset + 4];
                let gray = (77 * u32::from(color[0])
                    + 150 * u32::from(color[1])
                    + 29 * u32::from(color[2])
                    + 128)
                    >> 8;
                pixels[target_y * pixel_width + target_x] =
                    ((gray * u32::from(color[3]) + 255 * (255 - u32::from(color[3])) + 127) / 255)
                        as u8;
            }
        }
    }
    Ok(pixels)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", feature = "portable-pdf")))]
pub(crate) fn render(_path: &Path, _page_number: Option<u32>, _media: Media) -> Result<Vec<u8>> {
    anyhow::bail!("PDF rendering currently supports macOS and Linux")
}

#[cfg(all(
    test,
    any(target_os = "macos", target_os = "linux", feature = "portable-pdf")
))]
mod tests {
    use super::*;
    use crate::profile::NELKO;
    const WIDTH: usize = 812;
    const HEIGHT: usize = 1218;
    use std::io::Write;

    // An asymmetric 4x6 PDF: a black square only in its upper-left corner.
    // Constructed here so tests never include a user's real shipping label.
    fn fixture(pages: usize) -> Result<tempfile::NamedTempFile> {
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            format!(
                "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                (0..pages)
                    .map(|i| format!("{} 0 R", 3 + i * 2))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ];
        for i in 0..pages {
            objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 288 432] /Resources << >> /Contents {} 0 R >>", 4 + i * 2));
            let stream = "0 g\n0 396 36 36 re f\n";
            objects.push(format!(
                "<< /Length {} >>\nstream\n{stream}endstream",
                stream.len()
            ));
        }
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (i, object) in objects.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
        }
        let xref = bytes.len();
        bytes.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for offset in offsets.iter().skip(1) {
            bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(&bytes)?;
        Ok(file)
    }

    #[test]
    fn renders_upright_with_white_background() -> Result<()> {
        let file = fixture(1)?;
        let pixels = render(file.path(), None, NELKO.media)?;
        assert_eq!(pixels.len(), WIDTH * HEIGHT);
        assert_eq!(
            pixels[30 * WIDTH + 30],
            0,
            "upper-left square must be black"
        );
        assert_eq!(
            pixels[(HEIGHT - 30) * WIDTH + 30],
            255,
            "bottom must be white"
        );
        assert_eq!(
            pixels[30 * WIDTH + WIDTH - 30],
            255,
            "upper-right must be white"
        );
        Ok(())
    }

    #[test]
    fn requires_explicit_page_selection_for_multi_page_files() -> Result<()> {
        let file = fixture(2)?;
        assert!(render(file.path(), None, NELKO.media).is_err());
        assert!(render(file.path(), Some(3), NELKO.media).is_err());
        assert_eq!(
            render(file.path(), Some(2), NELKO.media)?.len(),
            WIDTH * HEIGHT
        );
        Ok(())
    }

    #[test]
    fn renders_at_the_selected_profiles_resolution() -> Result<()> {
        let file = fixture(1)?;
        let media = Media {
            dpi: 300,
            ..NELKO.media
        };
        let pixels = render(file.path(), None, media)?;
        let (width, height) = media.dimensions();
        assert_eq!((width, height), (1200, 1800));
        assert_eq!(pixels.len(), width * height);
        assert_eq!(
            pixels[140 * width + 140],
            0,
            "half-inch square must scale to 150 dots at 300 dpi"
        );
        assert_eq!(pixels[170 * width + 170], 255);
        Ok(())
    }
}
