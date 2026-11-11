use crate::app::agent::ChipElement;
use crate::prompt_images::{PastedImage, PromptImagePreview};
use aster_ratatui_textarea::{ElementId, ElementKind};
use ratatui::{
    layout::Alignment,
    style::Style,
    text::{Line, Span},
};
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::PathBuf};

#[derive(Serialize, Deserialize)]
enum Align {
    Left,
    Center,
    Right,
}

#[derive(Serialize, Deserialize)]
struct Display {
    spans: Vec<(String, Style)>,
    style: Style,
    alignment: Option<Align>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Chip {
    range: Range<usize>,
    kind: u16,
    display: Option<Display>,
}

impl From<ChipElement> for Chip {
    fn from(chip: ChipElement) -> Self {
        Self {
            range: chip.range,
            kind: chip.kind.0,
            display: chip.display.map(|line| Display {
                spans: line
                    .spans
                    .into_iter()
                    .map(|s| (s.content.into_owned(), s.style))
                    .collect(),
                style: line.style,
                alignment: line.alignment.map(|a| match a {
                    Alignment::Left => Align::Left,
                    Alignment::Center => Align::Center,
                    Alignment::Right => Align::Right,
                }),
            }),
        }
    }
}
impl From<Chip> for ChipElement {
    fn from(chip: Chip) -> Self {
        Self {
            range: chip.range,
            kind: ElementKind(chip.kind),
            display: chip.display.map(|line| Line {
                spans: line
                    .spans
                    .into_iter()
                    .map(|(text, style)| Span::styled(text, style))
                    .collect(),
                style: line.style,
                alignment: line.alignment.map(|a| match a {
                    Align::Left => Alignment::Left,
                    Align::Center => Alignment::Center,
                    Align::Right => Alignment::Right,
                }),
            }),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Image {
    display_number: usize,
    mime_type: String,
    dimensions: Option<(u32, u32)>,
    byte_len: usize,
    source_path: Option<PathBuf>,
    path: PathBuf,
}

pub(crate) mod images {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        images: &[PastedImage],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let entries = images
            .iter()
            .map(|image| {
                Ok(Image {
                    display_number: image.display_number,
                    mime_type: image.mime_type.clone(),
                    dimensions: image.dimensions,
                    byte_len: image.byte_len,
                    source_path: image.source_path.clone(),
                    path: image
                        .session_image_path
                        .clone()
                        .ok_or_else(|| serde::ser::Error::custom("Image has no durable path"))?,
                })
            })
            .collect::<Result<Vec<_>, S::Error>>()?;
        entries.serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PastedImage>, D::Error> {
        Ok(Vec::<Image>::deserialize(deserializer)?
            .into_iter()
            .map(|image| PastedImage {
                element_id: ElementId::from_raw(0),
                display_number: image.display_number,
                mime_type: image.mime_type,
                dimensions: image.dimensions,
                byte_len: image.byte_len,
                encoded_bytes: None,
                source_path: image.source_path,
                staged_temp_path: None,
                session_image_path: Some(image.path),
                preview: PromptImagePreview::default(),
            })
            .collect())
    }
}
