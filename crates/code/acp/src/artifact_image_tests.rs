#![cfg(test)]

use super::*;

fn document(content: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180">{content}</svg>"#)
}

#[test]
fn produces_png_with_the_svg_dimensions_and_internal_gradient() {
    let svg = document(
        r##"<defs><linearGradient id="bar"><stop stop-color="#234567"/><stop offset="1" stop-color="#abc"/></linearGradient></defs><rect x="20" y="20" width="180" height="80" fill="url(#bar)"/><text x="20" y="150" font-size="20">Quarter 1: +10%</text>"##,
    );
    let png = render_png(&svg).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 320);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 180);
    let explicit = r#"<svg xmlns="http://www.w3.org/2000/svg" width="240px" height="120" viewBox="0 0 320 180"><rect width="50" height="50"/></svg>"#;
    let png = render_png(explicit).unwrap();
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 240);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 120);
}

#[test]
fn refuses_external_resources_and_unsafe_xml_before_rendering() {
    for content in [
        r#"<image href="file:///private/sentinel.png"/>"#,
        r#"<image href="https://example.com/image.png"/>"#,
        r#"<image href="relative.png"/>"#,
        r#"<image href="data:image/png;base64,AAAA"/>"#,
        r#"<a href="https://example.com"><text>link</text></a>"#,
        r#"<g xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="file:///private/sentinel"/>"#,
        r#"<rect fill="url(file:///private/sentinel)"/>"#,
        r#"<rect fill="u&#114;l(https://example.com/image)"/>"#,
        r#"<rect style="fill:url(file:///private/sentinel)"/>"#,
        r#"<style>@import url(https://example.com/styles);</style>"#,
        r##"<use href="#recursive"/>"##,
        r#"<filter/>"#,
    ] {
        assert!(
            render_png(&document(content)).is_err(),
            "accepted {content}"
        );
    }
    for prefix in [
        r#"<!DOCTYPE svg [<!ENTITY resource SYSTEM "file:///private/sentinel">]>"#,
        r#"<?xml-stylesheet href="https://example.com/styles"?>"#,
    ] {
        assert!(render_png(&format!("{prefix}{}", document(""))).is_err());
    }
}

#[test]
fn refuses_empty_malformed_oversized_and_excessively_complex_svg() {
    for svg in [
        "".to_string(),
        "<svg".to_string(),
        "<not-svg/>".to_string(),
        document("").repeat(2),
        document(&" ".repeat(MAX_SVG_BYTES)),
        document(&"<g>".repeat(33)),
        document(&"<rect/>".repeat(2048)),
        format!(
            r#"<svg width="10" height="10" {}/>"#,
            (0..65)
                .map(|index| format!("a{index}=\"1\" "))
                .collect::<String>()
        ),
        r#"<svg width="4097" height="1"/>"#.to_string(),
        r#"<svg width="4096" height="4096"/>"#.to_string(),
        r#"<svg xmlns:x="urn:test" width="999999" height="999999" x:width="10" x:height="10"/>"#
            .to_string(),
        r#"<svg width="0" height="1"/>"#.to_string(),
        r#"<svg width="NaN" height="1"/>"#.to_string(),
        r#"<svg width="10%" height="20%"/>"#.to_string(),
        r#"<svg width="10" viewBox="0 0 10 10"/>"#.to_string(),
        r#"<svg viewBox="0 0 1e999 10"/>"#.to_string(),
        r#"<svg viewBox="0 0 1 2 3"/>"#.to_string(),
    ] {
        assert!(render_png(&svg).is_err(), "accepted {svg:.120}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_system_fallback_renders_distinct_chinese_glyphs() {
    let text = |value| {
        document(&format!(
            r#"<text x="20" y="90" font-size="48">{value}</text>"#
        ))
    };
    let blank = render_png(&text("")).unwrap();
    let zhong = render_png(&text("中")).unwrap();
    let wen = render_png(&text("文")).unwrap();
    assert_ne!(zhong, blank, "Chinese glyphs must not disappear");
    assert_ne!(
        zhong, wen,
        "Chinese glyphs must not become identical missing-glyph boxes"
    );
}
