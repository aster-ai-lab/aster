use a3s_code_mermaid::{rasterize, RenderParams, Rgba};
use anyhow::{ensure, Context, Result};
use quick_xml::{events::Event, Reader};

const MAX_SVG_BYTES: usize = 64 * 1024;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: f64 = 8_000_000.0;
const MAX_PNG_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn render_png(svg: &str) -> Result<Vec<u8>> {
    ensure!(!svg.trim().is_empty(), "PNG requires SVG content");
    ensure!(svg.len() <= MAX_SVG_BYTES, "SVG exceeds 64 KiB");
    validate_svg(svg)?;
    let rendered = rasterize(
        svg,
        &RenderParams {
            target_width_px: 0,
            max_height_px: MAX_DIMENSION,
            background: Some(Rgba::new(255, 255, 255, 255)),
            ..RenderParams::default()
        },
    )
    .context("SVG could not be rendered as PNG")?;
    ensure!(
        rendered.width_px <= MAX_DIMENSION
            && rendered.height_px <= MAX_DIMENSION
            && f64::from(rendered.width_px) * f64::from(rendered.height_px) <= MAX_PIXELS,
        "PNG exceeds the 4096-pixel axis or 8-megapixel limit"
    );
    ensure!(rendered.png.len() <= MAX_PNG_BYTES, "PNG exceeds 16 MiB");
    Ok(rendered.png)
}

fn validate_svg(svg: &str) -> Result<()> {
    let mut reader = Reader::from_str(svg);
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut root_seen = false;
    loop {
        let event = reader.read_event().context("invalid SVG XML")?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                elements += 1;
                ensure!(
                    elements <= 2048 && depth < 32,
                    "SVG complexity exceeds limit"
                );
                let name = start.local_name();
                ensure!(
                    !matches!(
                        name.as_ref(),
                        b"image" | b"use" | b"style" | b"script" | b"foreignObject" | b"filter"
                    ),
                    "SVG resource, script, and filter elements are unsupported"
                );
                let root = depth == 0;
                if root {
                    ensure!(
                        !root_seen && name.as_ref() == b"svg",
                        "SVG requires one svg root"
                    );
                    root_seen = true;
                }
                let (mut width, mut height, mut view_box) = (None, None, None);
                for (index, attribute) in start.attributes().enumerate() {
                    ensure!(index < 64, "SVG attribute count exceeds limit");
                    let attribute = attribute.context("invalid SVG attribute")?;
                    let key = attribute.key.local_name();
                    let value = attribute.decode_and_unescape_value(reader.decoder())?;
                    ensure!(
                        key.as_ref() != b"style",
                        "SVG inline CSS is unsupported; use presentation attributes"
                    );
                    if key.as_ref() == b"href" {
                        ensure!(
                            value.starts_with('#') && value.len() > 1,
                            "SVG external references are forbidden"
                        );
                    }
                    let lower = value.to_ascii_lowercase();
                    if lower.contains("url(") {
                        let fragment = lower
                            .trim()
                            .strip_prefix("url(#")
                            .and_then(|value| value.strip_suffix(')'));
                        ensure!(
                            fragment.is_some_and(|id| !id.is_empty()
                                && !id
                                    .chars()
                                    .any(|ch| ch.is_whitespace() || "'\"()".contains(ch))),
                            "SVG external references are forbidden"
                        );
                    }
                    if root {
                        match attribute.key.as_ref() {
                            b"width" => width = Some(dimension(&value)?),
                            b"height" => height = Some(dimension(&value)?),
                            b"viewBox" => {
                                let numbers = value
                                    .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
                                    .filter(|part| !part.is_empty())
                                    .map(str::parse::<f64>)
                                    .collect::<std::result::Result<Vec<_>, _>>()?;
                                ensure!(
                                    numbers.len() == 4
                                        && numbers.iter().all(|value| value.is_finite()),
                                    "SVG viewBox must contain four finite numbers"
                                );
                                view_box = Some((numbers[2], numbers[3]));
                            }
                            _ => {}
                        }
                    }
                }
                if root {
                    let size = match (width, height, view_box) {
                        (Some(width), Some(height), _) => (width, height),
                        (None, None, Some(size)) => size,
                        _ => anyhow::bail!("SVG requires numeric width and height, or a viewBox without width and height"),
                    };
                    ensure!(
                        size.0 >= 1.0 && size.1 >= 1.0
                            && size.0 <= f64::from(MAX_DIMENSION) && size.1 <= f64::from(MAX_DIMENSION)
                            && size.0 * size.1 <= MAX_PIXELS,
                        "SVG exceeds the 4096-pixel axis or 8-megapixel limit, or has invalid dimensions"
                    );
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).context("invalid SVG nesting")?;
            }
            Event::DocType(_) | Event::PI(_) => {
                anyhow::bail!("SVG DTDs and processing instructions are forbidden")
            }
            Event::Text(text) if depth == 0 => ensure!(
                text.iter().all(u8::is_ascii_whitespace),
                "text outside SVG root is forbidden"
            ),
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(root_seen && depth == 0, "SVG root is missing or incomplete");
    Ok(())
}

fn dimension(value: &str) -> Result<f64> {
    let value = value.trim();
    let number = value.strip_suffix("px").unwrap_or(value).parse::<f64>()?;
    ensure!(number.is_finite(), "SVG dimensions must be finite");
    Ok(number)
}

#[cfg(test)]
#[path = "artifact_image_tests.rs"]
mod tests;
