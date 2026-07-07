//! Builds real Word/Excel/PowerPoint files from a lightweight text format the
//! model can produce via a single WRITE-style trigger parameter (see
//! CREATE_DOCX / CREATE_XLSX / CREATE_PPTX in parser.rs and
//! resources/skills/llm-functions-v1.md for the exact conventions), and
//! reads them back for the file viewer's inline preview (`read_office_preview`
//! below, used by commands::files::file_read_office_preview).
//!
//! DOCX and XLSX go through mature, widely-used crates (docx-rs,
//! rust_xlsxwriter for writing; docx-rs, calamine for reading). PPTX has no
//! equally established Rust crate, so both its writer and reader below
//! hand-build/parse the OOXML directly — covered by structural tests and,
//! for reading, cross-checked against files produced by real python-pptx
//! (not just our own writer) but not verified against actual PowerPoint.

use std::io::{Cursor, Read, Write};
use docx_rs::{Docx, DocumentChild, Paragraph, ParagraphChild, Run, RunChild};
use rust_xlsxwriter::{Format, Workbook};
use serde::Serialize;
use zip::write::SimpleFileOptions;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OfficePreview {
    Docx(DocxPreview),
    Xlsx(XlsxPreview),
    Pptx(PptxPreview),
}

/// Dispatches to the right reader based on file extension. Returns `Err`
/// for unrecognized extensions or files that fail to parse.
pub fn read_office_preview(rel_path: &str, bytes: &[u8]) -> Result<OfficePreview, String> {
    let lower = rel_path.to_lowercase();
    if lower.ends_with(".docx") {
        read_docx(bytes).map(OfficePreview::Docx)
    } else if lower.ends_with(".xlsx") {
        read_xlsx(bytes).map(OfficePreview::Xlsx)
    } else if lower.ends_with(".pptx") {
        read_pptx(bytes).map(OfficePreview::Pptx)
    } else {
        Err(format!("Unsupported file type for office preview: {rel_path}"))
    }
}

/// Builds a .docx from a small markdown-like subset: "# "/"## "/"### "
/// headings, "- " bullet lines, blank-line-separated paragraphs otherwise.
/// Models sometimes go a level deeper than "##" in a long structured
/// document (business plans, reports); anything past three "#" collapses
/// to the third tier rather than falling through to a literal "#### Foo"
/// paragraph — real markdown supports six levels, but three is already
/// generous for a Word document and keeps this simple.
pub fn build_docx(content: &str) -> Result<Vec<u8>, String> {
    let mut docx = Docx::new();

    for line in content.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        // Named styles like "Heading1" only render correctly if they're
        // registered via add_style — docx-rs doesn't ship Word's built-in
        // style definitions, so an unregistered style ID is a dangling
        // reference Word silently ignores. Direct run formatting has no
        // such dependency.
        let paragraph = if let Some(text) = heading_text(trimmed, 1) {
            Paragraph::new().add_run(Run::new().add_text(text).bold().size(32))
        } else if let Some(text) = heading_text(trimmed, 2) {
            Paragraph::new().add_run(Run::new().add_text(text).bold().size(28))
        } else if let Some(text) = heading_text(trimmed, 3) {
            Paragraph::new().add_run(Run::new().add_text(text).bold().size(24))
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            Paragraph::new().add_run(Run::new().add_text(format!("\u{2022} {text}")))
        } else {
            Paragraph::new().add_run(Run::new().add_text(trimmed))
        };
        docx = docx.add_paragraph(paragraph);
    }

    let mut buf = Cursor::new(Vec::new());
    docx.build().pack(&mut buf).map_err(|e| format!("Failed to build docx: {e}"))?;
    Ok(buf.into_inner())
}

/// Matches a markdown heading line at exactly `level` "#" characters
/// (level 3 also absorbs anything deeper, i.e. "####"+), returning the text
/// after the marker. `"## "` must not match at level 1 — checked via the
/// character immediately following the run of "#"s not being another "#".
fn heading_text(line: &str, level: usize) -> Option<&str> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || !line[hashes..].starts_with(' ') {
        return None;
    }
    let matches_level = if level == 3 { hashes >= 3 } else { hashes == level };
    matches_level.then(|| line[hashes + 1..].trim_start())
}

#[derive(Debug, Serialize)]
pub struct DocxPreview {
    pub blocks: Vec<DocxBlock>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocxBlock {
    Heading1 { text: String },
    Heading2 { text: String },
    Heading3 { text: String },
    Bullet { text: String },
    Paragraph { text: String },
}

/// Extracts a lightweight structural preview from a real .docx file — ours
/// or one produced by actual Word/Google Docs/LibreOffice. Heading/bullet
/// detection has two paths since documents get there two different ways:
/// a named paragraph style (`w:pStyle val="Heading1"` etc, what real word
/// processors emit for their built-in styles) or direct run formatting
/// (bold+size on the first run, what `build_docx` above writes, since
/// named styles would be dangling references unless separately registered).
pub fn read_docx(bytes: &[u8]) -> Result<DocxPreview, String> {
    let docx = docx_rs::read_docx(bytes).map_err(|e| format!("Could not parse .docx: {e}"))?;

    let mut blocks = Vec::new();
    for child in &docx.document.children {
        let DocumentChild::Paragraph(p) = child else { continue };

        let mut text = String::new();
        for pc in &p.children {
            let ParagraphChild::Run(r) = pc else { continue };
            for rc in &r.children {
                if let RunChild::Text(t) = rc {
                    text.push_str(&t.text);
                }
            }
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }

        let style_id = p.property.style.as_ref().map(|s| s.val.as_str()).unwrap_or("");
        let style_lower = style_id.to_lowercase();

        let first_run_props = p.children.iter().find_map(|pc| {
            if let ParagraphChild::Run(r) = pc { Some(&r.run_property) } else { None }
        });
        let bold = first_run_props
            .and_then(|rp| serde_json::to_value(&rp.bold).ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let size = first_run_props
            .and_then(|rp| serde_json::to_value(&rp.sz).ok())
            .and_then(|v| v.as_u64());

        let block = if style_lower.starts_with("heading1") || style_lower == "title" {
            DocxBlock::Heading1 { text }
        } else if style_lower.starts_with("heading2") {
            DocxBlock::Heading2 { text }
        } else if style_lower.starts_with("heading3") {
            DocxBlock::Heading3 { text }
        } else if bold && size.is_some_and(|s| s >= 32) {
            DocxBlock::Heading1 { text }
        } else if bold && size.is_some_and(|s| s >= 28) {
            DocxBlock::Heading2 { text }
        } else if bold && size.is_some_and(|s| s >= 24) {
            DocxBlock::Heading3 { text }
        } else if style_lower.contains("bullet") || style_lower.contains("list") || p.has_numbering {
            DocxBlock::Bullet { text }
        } else if let Some(rest) = text.strip_prefix('\u{2022}') {
            DocxBlock::Bullet { text: rest.trim().to_string() }
        } else {
            DocxBlock::Paragraph { text }
        };
        blocks.push(block);
    }

    Ok(DocxPreview { blocks })
}

/// Builds an .xlsx from CSV-like text: the first non-empty line is a bold
/// header row, every following line is a data row. Cells are comma
/// separated with optional double-quoting (`"a, b"`) for values containing
/// a literal comma; numeric-looking cells are written as numbers.
pub fn build_xlsx(content: &str) -> Result<Vec<u8>, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let header_format = Format::new().set_bold();

    let mut row_idx = 0u32;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cells = parse_csv_line(line);
        for (col_idx, cell) in cells.iter().enumerate() {
            let col_idx = col_idx as u16;
            if row_idx == 0 {
                worksheet.write_with_format(row_idx, col_idx, cell.as_str(), &header_format)
                    .map_err(|e| format!("Failed to write header cell: {e}"))?;
            } else if let Ok(n) = cell.parse::<f64>() {
                worksheet.write_number(row_idx, col_idx, n)
                    .map_err(|e| format!("Failed to write number cell: {e}"))?;
            } else {
                worksheet.write_string(row_idx, col_idx, cell.as_str())
                    .map_err(|e| format!("Failed to write string cell: {e}"))?;
            }
        }
        row_idx += 1;
    }

    workbook.save_to_buffer().map_err(|e| format!("Failed to build xlsx: {e}"))
}

#[derive(Debug, Serialize)]
pub struct XlsxPreview {
    pub sheets: Vec<XlsxSheet>,
}

#[derive(Debug, Serialize)]
pub struct XlsxSheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

/// Reads every sheet of a real .xlsx file into plain string rows for
/// preview. Uses calamine rather than hand-parsed XML since spreadsheet
/// cell typing (shared strings, inline strings, numeric formats) has enough
/// edge cases that a mature reader is worth the dependency.
pub fn read_xlsx(bytes: &[u8]) -> Result<XlsxPreview, String> {
    use calamine::Reader;

    let cursor = Cursor::new(bytes);
    let mut workbook: calamine::Xlsx<_> = calamine::open_workbook_from_rs(cursor)
        .map_err(|e| format!("Could not parse .xlsx: {e}"))?;

    let mut sheets = Vec::new();
    for name in workbook.sheet_names() {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| format!("Could not read sheet \"{name}\": {e}"))?;
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(|cell| cell.to_string()).collect())
            .collect();
        sheets.push(XlsxSheet { name, rows });
    }

    Ok(XlsxPreview { sheets })
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

struct Slide {
    title: String,
    bullets: Vec<Bullet>,
}

struct Bullet {
    text: String,
    /// True for a line that used a markdown "#"+ sub-heading marker inside
    /// the slide body (e.g. "## Risques Techniques" under a slide titled
    /// "# Risques et Mitigation") — models writing a long structured deck
    /// often nest a second heading level per slide. Rendered bold with no
    /// bullet glyph instead of leaking the literal "##" as bullet text.
    heading: bool,
}

/// Builds a .pptx from slides separated by a line containing only "---".
/// Within each slide, a "# " line is the title and "- " lines are bullets.
pub fn build_pptx(content: &str) -> Result<Vec<u8>, String> {
    let slides = parse_slides(content);
    if slides.is_empty() {
        return Err("No slides found (separate slides with a line containing only ---)".into());
    }

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        write_entry(&mut zip, options, "[Content_Types].xml", &content_types_xml(slides.len()))?;
        write_entry(&mut zip, options, "_rels/.rels", ROOT_RELS)?;
        write_entry(&mut zip, options, "docProps/app.xml", &app_xml(slides.len()))?;
        write_entry(&mut zip, options, "docProps/core.xml", CORE_XML)?;
        write_entry(&mut zip, options, "ppt/presentation.xml", &presentation_xml(slides.len()))?;
        write_entry(&mut zip, options, "ppt/_rels/presentation.xml.rels", &presentation_rels_xml(slides.len()))?;
        write_entry(&mut zip, options, "ppt/theme/theme1.xml", THEME_XML)?;
        write_entry(&mut zip, options, "ppt/slideMasters/slideMaster1.xml", SLIDE_MASTER_XML)?;
        write_entry(&mut zip, options, "ppt/slideMasters/_rels/slideMaster1.xml.rels", SLIDE_MASTER_RELS)?;
        write_entry(&mut zip, options, "ppt/slideLayouts/slideLayout1.xml", SLIDE_LAYOUT_XML)?;
        write_entry(&mut zip, options, "ppt/slideLayouts/_rels/slideLayout1.xml.rels", SLIDE_LAYOUT_RELS)?;

        for (i, slide) in slides.iter().enumerate() {
            let n = i + 1;
            write_entry(&mut zip, options, &format!("ppt/slides/slide{n}.xml"), &slide_xml(slide))?;
            write_entry(&mut zip, options, &format!("ppt/slides/_rels/slide{n}.xml.rels"), SLIDE_RELS)?;
        }

        zip.finish().map_err(|e| format!("Failed to finalize pptx zip: {e}"))?;
    }
    Ok(buf.into_inner())
}

fn parse_slides(content: &str) -> Vec<Slide> {
    let mut slides = Vec::new();
    let mut title = String::new();
    let mut bullets: Vec<Bullet> = Vec::new();
    let mut started = false;

    let flush = |title: &mut String, bullets: &mut Vec<Bullet>, slides: &mut Vec<Slide>| {
        if !title.is_empty() || !bullets.is_empty() {
            slides.push(Slide { title: std::mem::take(title), bullets: std::mem::take(bullets) });
        }
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            flush(&mut title, &mut bullets, &mut slides);
            started = false;
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        started = true;
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if hashes > 0 && trimmed[hashes..].starts_with(' ') {
            let text = trimmed[hashes..].trim_start().to_string();
            // A single "#" becomes this slide's title, same as before — but
            // only the first one; once a title exists, any further "#"-led
            // line (single or nested) is a sub-heading bullet rather than
            // silently overwriting the title.
            if hashes == 1 && title.is_empty() {
                title = text;
            } else {
                bullets.push(Bullet { text, heading: true });
            }
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            bullets.push(Bullet { text: text.to_string(), heading: false });
        } else if title.is_empty() {
            title = trimmed.to_string();
        } else {
            bullets.push(Bullet { text: trimmed.to_string(), heading: false });
        }
    }
    if started {
        flush(&mut title, &mut bullets, &mut slides);
    }
    slides
}

#[derive(Debug, Serialize)]
pub struct PptxPreview {
    pub slides: Vec<PptxSlide>,
}

#[derive(Debug, Serialize)]
pub struct PptxSlide {
    pub title: String,
    pub bullets: Vec<PptxBullet>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct PptxBullet {
    pub text: String,
    pub heading: bool,
}

/// Reads slide title/bullet text back out of a real .pptx (ours or one
/// produced by PowerPoint/Keynote/Google Slides): walks each
/// `ppt/slides/slideN.xml` part looking for `<p:sp>` shapes, treating the
/// shape whose `<p:ph type="title|ctrTitle">` marks it as the title and
/// every other shape's paragraphs as bullet lines. This only recovers text
/// content, not layout/images/formatting — good enough for a quick preview.
pub fn read_pptx(bytes: &[u8]) -> Result<PptxPreview, String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Could not open .pptx: {e}"))?;

    let mut slide_paths: Vec<(u32, String)> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .filter_map(|name| {
            let rest = name.strip_prefix("ppt/slides/slide")?;
            let n: u32 = rest.strip_suffix(".xml")?.parse().ok()?;
            Some((n, name))
        })
        .collect();
    slide_paths.sort_by_key(|(n, _)| *n);

    let mut slides = Vec::new();
    for (_, path) in slide_paths {
        let mut file = archive.by_name(&path).map_err(|e| format!("Could not read {path}: {e}"))?;
        let mut xml = String::new();
        file.read_to_string(&mut xml).map_err(|e| format!("Could not read {path}: {e}"))?;
        slides.push(parse_slide_text(&xml));
    }

    if slides.is_empty() {
        return Err("No slides found in this .pptx".into());
    }
    Ok(PptxPreview { slides })
}

fn parse_slide_text(xml: &str) -> PptxSlide {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader as XmlReader;

    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut title = String::new();
    let mut bullets: Vec<PptxBullet> = Vec::new();

    let mut in_sp = false;
    let mut sp_is_title = false;
    let mut sp_paragraphs: Vec<(String, bool)> = Vec::new();
    let mut cur_paragraph = String::new();
    let mut cur_paragraph_bold = false;
    let mut in_text = false;

    loop {
        // `<p:ph type="title"/>` and `<a:rPr b="1"/>` are both always empty
        // (no child elements in practice), so quick-xml reports them as
        // `Event::Empty`, not a `Start`/`End` pair — a real PowerPoint/
        // python-pptx export hits this path; only our own hand-written
        // slide_xml above happens to always emit `<a:rPr b="1"/>` the same
        // way, so this must be handled explicitly rather than only on
        // `Event::Start`.
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"p:sp" => {
                    in_sp = true;
                    sp_is_title = false;
                    sp_paragraphs.clear();
                }
                b"p:ph" if in_sp => {
                    if is_title_ph(&e) {
                        sp_is_title = true;
                    }
                }
                b"a:p" => {
                    cur_paragraph.clear();
                    cur_paragraph_bold = false;
                }
                b"a:rPr" if in_sp => {
                    if is_bold_rpr(&e) {
                        cur_paragraph_bold = true;
                    }
                }
                b"a:t" => in_text = true,
                _ => {}
            },
            Ok(Event::Empty(e)) if in_sp => match e.name().as_ref() {
                b"p:ph" => {
                    if is_title_ph(&e) {
                        sp_is_title = true;
                    }
                }
                b"a:rPr" => {
                    if is_bold_rpr(&e) {
                        cur_paragraph_bold = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) if in_text => {
                if let Ok(raw) = t.decode() {
                    match quick_xml::escape::unescape(&raw) {
                        Ok(unescaped) => cur_paragraph.push_str(&unescaped),
                        Err(_) => cur_paragraph.push_str(&raw),
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"a:t" => in_text = false,
                b"a:p" if in_sp => {
                    let text = cur_paragraph.trim().to_string();
                    if !text.is_empty() {
                        sp_paragraphs.push((text, cur_paragraph_bold));
                    }
                }
                b"p:sp" => {
                    if sp_is_title {
                        title = sp_paragraphs.drain(..).map(|(t, _)| t).collect::<Vec<_>>().join(" ");
                    } else {
                        bullets.extend(sp_paragraphs.drain(..).map(|(text, heading)| PptxBullet { text, heading }));
                    }
                    in_sp = false;
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    PptxSlide { title, bullets }
}

fn is_title_ph(e: &quick_xml::events::BytesStart) -> bool {
    e.attributes().flatten().any(|a| {
        a.key.as_ref() == b"type"
            && matches!(
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0).as_deref(),
                Ok("title") | Ok("ctrTitle")
            )
    })
}

fn is_bold_rpr(e: &quick_xml::events::BytesStart) -> bool {
    e.attributes().flatten().any(|a| {
        a.key.as_ref() == b"b"
            && matches!(
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0).as_deref(),
                Ok("1") | Ok("true")
            )
    })
}

fn write_entry<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    content: &str,
) -> Result<(), String> {
    zip.start_file(name, options).map_err(|e| format!("Failed to start {name}: {e}"))?;
    zip.write_all(content.as_bytes()).map_err(|e| format!("Failed to write {name}: {e}"))?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

const CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:creator>Open Atelier</dc:creator>
</cp:coreProperties>"#;

const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Open Atelier">
  <a:themeElements>
    <a:clrScheme name="Open Atelier">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="C55A11"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Open Atelier">
      <a:majorFont><a:latin typeface="Calibri"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Open Atelier">
      <a:fillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:fillStyleLst>
      <a:lnStyleLst>
        <a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
      </a:lnStyleLst>
      <a:effectStyleLst>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
      </a:effectStyleLst>
      <a:bgFillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>"#;

const SLIDE_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/>
  </p:sldLayoutIdLst>
</p:sldMaster>"#;

const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

const SLIDE_LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="title" preserve="1">
  <p:cSld name="Title and Content">
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:overrideClrMapping bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>
</p:sldLayout>"#;

const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

const SLIDE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;

fn content_types_xml(slide_count: usize) -> String {
    let mut overrides = String::new();
    for i in 1..=slide_count {
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
  {overrides}
</Types>"#
    )
}

fn app_xml(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Open Atelier</Application>
  <Slides>{slide_count}</Slides>
</Properties>"#
    )
}

fn presentation_xml(slide_count: usize) -> String {
    let mut sld_id_lst = String::new();
    for i in 0..slide_count {
        let id = 256 + i;
        let r_id = i + 2; // rId1 is the slide master
        sld_id_lst.push_str(&format!(r#"<p:sldId id="{id}" r:id="rId{r_id}"/>"#));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
  <p:sldIdLst>{sld_id_lst}</p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#
    )
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let mut rels = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for i in 0..slide_count {
        let n = i + 1;
        let r_id = i + 2;
        rels.push_str(&format!(
            r#"<Relationship Id="rId{r_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#
        ));
    }
    let theme_r_id = slide_count + 2;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{theme_r_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#
    ));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    )
}

fn slide_xml(slide: &Slide) -> String {
    let title = xml_escape(&slide.title);
    let mut body_paragraphs = String::new();
    for bullet in &slide.bullets {
        if bullet.heading {
            // A sub-heading reads as a section label, not a bullet point —
            // bold, no indent/bullet glyph, matching the semantic diff from
            // an ordinary "- " bullet.
            body_paragraphs.push_str(&format!(
                r#"<a:p><a:pPr><a:buNone/></a:pPr><a:r><a:rPr b="1"/><a:t>{}</a:t></a:r></a:p>"#,
                xml_escape(&bullet.text)
            ));
        } else {
            body_paragraphs.push_str(&format!(
                r#"<a:p><a:pPr marL="285750" indent="-285750"><a:buChar char="&#8226;"/></a:pPr><a:r><a:t>{}</a:t></a:r></a:p>"#,
                xml_escape(&bullet.text)
            ));
        }
    }
    if body_paragraphs.is_empty() {
        body_paragraphs = "<a:p/>".to_string();
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="457200" y="274638"/><a:ext cx="11277600" cy="1143000"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/><a:lstStyle/>
          <a:p><a:r><a:t>{title}</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="body" idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="11277600" cy="4800600"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/><a:lstStyle/>
          {body_paragraphs}
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_builds_nonempty_zip() {
        let bytes = build_docx("# Title\n\nSome paragraph.\n\n- bullet one\n- bullet two").unwrap();
        assert!(!bytes.is_empty());
        // A docx is a zip; the local file header signature is "PK\x03\x04".
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
    }

    #[test]
    fn xlsx_builds_nonempty_zip() {
        let bytes = build_xlsx("Name,Age\nAlice,30\nBob,25").unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
    }

    #[test]
    fn csv_line_handles_quoted_commas() {
        let cells = parse_csv_line(r#"Alice,"Doe, Jr.",30"#);
        assert_eq!(cells, vec!["Alice", "Doe, Jr.", "30"]);
    }

    #[test]
    fn pptx_builds_valid_zip_with_all_parts() {
        let bytes = build_pptx("# Slide One\n- point a\n- point b\n---\n# Slide Two\n- point c").unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");

        let reader = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
        ] {
            assert!(names.contains(&required.to_string()), "missing part: {required}");
        }
    }

    #[test]
    fn pptx_rejects_empty_content() {
        assert!(build_pptx("").is_err());
    }

    #[test]
    fn pptx_slide_xml_contains_title_and_bullets() {
        let slide = Slide {
            title: "Hello & Welcome".into(),
            bullets: vec![Bullet { text: "<point>".into(), heading: false }],
        };
        let xml = slide_xml(&slide);
        assert!(xml.contains("Hello &amp; Welcome"));
        assert!(xml.contains("&lt;point&gt;"));
    }

    #[test]
    fn pptx_slide_xml_renders_sub_heading_bold_without_bullet_glyph() {
        let slide = Slide {
            title: "Risques".into(),
            bullets: vec![Bullet { text: "Risques Techniques".into(), heading: true }],
        };
        let xml = slide_xml(&slide);
        assert!(xml.contains(r#"<a:rPr b="1"/>"#));
        assert!(xml.contains("<a:buNone/>"));
        assert!(!xml.contains("## Risques Techniques"));
    }

    #[test]
    fn read_docx_round_trips_headings_bullets_and_paragraphs() {
        let bytes = build_docx("# Title\n\nA plain paragraph.\n\n## Subheading\n\n- one\n- two").unwrap();
        let preview = read_docx(&bytes).unwrap();
        assert_eq!(
            preview.blocks,
            vec![
                DocxBlock::Heading1 { text: "Title".into() },
                DocxBlock::Paragraph { text: "A plain paragraph.".into() },
                DocxBlock::Heading2 { text: "Subheading".into() },
                DocxBlock::Bullet { text: "one".into() },
                DocxBlock::Bullet { text: "two".into() },
            ]
        );
    }

    #[test]
    fn read_docx_round_trips_heading3_and_deeper() {
        let bytes = build_docx("### Level three\n\n#### Level four collapses to three").unwrap();
        let preview = read_docx(&bytes).unwrap();
        assert_eq!(
            preview.blocks,
            vec![
                DocxBlock::Heading3 { text: "Level three".into() },
                DocxBlock::Heading3 { text: "Level four collapses to three".into() },
            ]
        );
    }

    #[test]
    fn read_xlsx_round_trips_header_and_rows() {
        let bytes = build_xlsx("Name,Age\nAlice,30\nBob,25").unwrap();
        let preview = read_xlsx(&bytes).unwrap();
        assert_eq!(preview.sheets.len(), 1);
        assert_eq!(
            preview.sheets[0].rows,
            vec![
                vec!["Name".to_string(), "Age".to_string()],
                vec!["Alice".to_string(), "30".to_string()],
                vec!["Bob".to_string(), "25".to_string()],
            ]
        );
    }

    #[test]
    fn read_pptx_round_trips_titles_and_bullets() {
        let bytes = build_pptx("# Slide One\n- point a\n- point b\n---\n# Slide Two\n- point c").unwrap();
        let preview = read_pptx(&bytes).unwrap();
        assert_eq!(preview.slides.len(), 2);
        assert_eq!(preview.slides[0].title, "Slide One");
        assert_eq!(preview.slides[0].bullets, vec![
            PptxBullet { text: "point a".into(), heading: false },
            PptxBullet { text: "point b".into(), heading: false },
        ]);
        assert_eq!(preview.slides[1].title, "Slide Two");
        assert_eq!(preview.slides[1].bullets, vec![PptxBullet { text: "point c".into(), heading: false }]);
    }

    #[test]
    fn read_pptx_round_trips_nested_sub_heading_as_bold_bullet() {
        let bytes = build_pptx("# Risques et Mitigation\n## Risques Techniques\n- Retards\n## Risques Commerciaux\n- Concurrence").unwrap();
        let preview = read_pptx(&bytes).unwrap();
        assert_eq!(preview.slides.len(), 1);
        assert_eq!(preview.slides[0].title, "Risques et Mitigation");
        assert_eq!(preview.slides[0].bullets, vec![
            PptxBullet { text: "Risques Techniques".into(), heading: true },
            PptxBullet { text: "Retards".into(), heading: false },
            PptxBullet { text: "Risques Commerciaux".into(), heading: true },
            PptxBullet { text: "Concurrence".into(), heading: false },
        ]);
        // The literal "##" markdown marker must never leak into bullet text.
        assert!(preview.slides[0].bullets.iter().all(|b| !b.text.starts_with('#')));
    }

    #[test]
    fn read_pptx_rejects_non_pptx_bytes() {
        assert!(read_pptx(b"not a zip").is_err());
    }

    #[test]
    fn read_xlsx_rejects_non_xlsx_bytes() {
        assert!(read_xlsx(b"not a zip").is_err());
    }

    #[test]
    fn read_docx_rejects_non_docx_bytes() {
        assert!(read_docx(b"not a zip").is_err());
    }
}
