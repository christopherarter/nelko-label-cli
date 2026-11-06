use crate::{profile::Language, raster::Label};

/// Encoders consume the same monochrome raster regardless of its transport.
pub(crate) trait LabelEncoder {
    fn encode(&self, label: &Label) -> Vec<u8>;
}

#[derive(Debug)]
struct Tspl;

impl LabelEncoder for Tspl {
    fn encode(&self, label: &Label) -> Vec<u8> {
        let media = label.media();
        let (_, height) = media.dimensions();
        let mut job = format!(
            "SIZE {},{}\r\nDIRECTION 1,0\r\nREFERENCE 0,0\r\nCLS\r\nBITMAP 0,0,{},{height},0,",
            media.width_inches,
            media.height_inches,
            label.row_bytes()
        )
        .into_bytes();
        job.extend_from_slice(label.bitmap());
        job.extend_from_slice(format!("\r\nPRINT 1,{}\r\n", label.copies()).as_bytes());
        job
    }
}

pub(crate) fn encode(language: Language, label: &Label) -> Vec<u8> {
    match language {
        Language::Tspl => Tspl.encode(label),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Media;

    #[test]
    fn uses_profile_dimensions_and_copies_in_commands() -> anyhow::Result<()> {
        let media = Media {
            width_inches: 2.0,
            height_inches: 3.0,
            dpi: 8,
        };
        let label = Label::from_gray(vec![255; 16 * 24], media, 160, 3)?;
        let job = encode(Language::Tspl, &label);
        assert!(job.starts_with(b"SIZE 2,3\r\n"));
        assert!(
            job.windows(b"BITMAP 0,0,2,24,0,".len())
                .any(|w| w == b"BITMAP 0,0,2,24,0,")
        );
        assert!(job.ends_with(b"\r\nPRINT 1,3\r\n"));
        Ok(())
    }
}
