//! Renders real HTML/CSS to a real PDF, for EXPORT_PDF (see parser.rs and
//! resources/skills/llm-functions-v1.md). Backed by printpdf's `html`
//! feature (on by default), which parses HTML with html5ever (the same
//! parser Firefox/Servo use) and lays it out with azul's CSS engine — real
//! fonts, colors, borders, padding, not just plain text — rather than
//! shelling out to an installed browser or wkhtmltopdf binary that may not
//! exist on the user's machine.

use printpdf::{GeneratePdfOptions, PdfDocument, PdfSaveOptions};
use std::collections::BTreeMap;

pub fn html_to_pdf(html: &str) -> Result<Vec<u8>, String> {
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .map_err(|e| format!("Failed to render HTML to PDF: {e}"))?;

    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    if bytes.is_empty() {
        return Err("Rendered PDF was empty".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_simple_html_to_a_real_pdf() {
        let bytes =
            html_to_pdf("<html><body><h1>Title</h1><p>Hello world.</p></body></html>").unwrap();
        assert!(!bytes.is_empty());
        // PDF files start with a version header comment like "%PDF-1.7".
        assert_eq!(&bytes[0..5], b"%PDF-");
    }

    #[test]
    fn renders_styled_html_with_css() {
        let html = r#"<html><head><style>.title{font-size:24px;color:#333333;}</style></head>
            <body><div class="title">Styled</div></body></html>"#;
        let bytes = html_to_pdf(html).unwrap();
        assert_eq!(&bytes[0..5], b"%PDF-");
    }
}
