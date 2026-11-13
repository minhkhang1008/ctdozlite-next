//! In-app document preview: extract readable text from office/PDF attachments.
//!
//! Session-only (no disk cache): the worker downloads bytes (bounded 10 MiB),
//! [`extract_text`] pulls plain text, and the GUI keeps the result in RAM.

/// Max preview text kept in memory / shown in the full viewer.
pub const MAX_PREVIEW_CHARS: usize = 20_000;
/// Max download size for a preview fetch (10 MiB).
pub const MAX_PREVIEW_BYTES: usize = 10 * 1024 * 1024;
/// Max spreadsheet rows rendered per sheet.
const MAX_SHEET_ROWS: usize = 50;

/// File types supported by the explicit text-preview action.
pub fn supports(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "pdf" | "docx" | "xlsx" | "pptx" | "txt" | "csv" | "md" | "log" | "rtf"
            )
        })
}

/// Extract readable text from raw file bytes by extension (no leading dot).
///
/// Supported: `pdf`, `docx`, `xlsx`, `pptx`, `txt`, `csv`, `md`, `log`, `rtf`
/// (RTF control words are stripped on a best-effort basis).
/// Legacy OLE files (`doc`, `xls`, `ppt`) return an error asking the user to
/// open them outside the app.
pub fn extract_text(bytes: &[u8], ext: &str) -> Result<String, String> {
    if bytes.len() > MAX_PREVIEW_BYTES {
        return Err("Tài liệu vượt giới hạn 10 MiB.".into());
    }
    let ext = ext.to_ascii_lowercase();
    if matches!(ext.as_str(), "docx" | "xlsx" | "pptx") {
        validate_archive(bytes)?;
    }
    let raw = match ext.as_str() {
        "pdf" => extract_pdf(bytes)?,
        "docx" => extract_docx(bytes)?,
        "xlsx" => extract_xlsx(bytes)?,
        "pptx" => extract_pptx(bytes)?,
        "txt" | "csv" | "md" | "log" => decode_lossy(bytes),
        "rtf" => strip_rtf(&decode_lossy(bytes)),
        "doc" | "xls" | "ppt" => {
            return Err("Định dạng cũ (.doc/.xls/.ppt), hãy mở ngoài.".to_string());
        }
        _ => return Err(format!("Không xem trước được .{ext}, hãy mở ngoài.")),
    };
    Ok(truncate_chars(&raw, MAX_PREVIEW_CHARS))
}

/// First `n` lines of `text` for the inline bubble snippet.
pub fn snippet_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

fn decode_lossy(bytes: &[u8]) -> String {
    // Strip UTF-8 BOM if present, then lossy-decode.
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}…")
}

fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes).map_err(|e| format!("Không đọc được PDF: {e}"))
}

const MAX_XML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;
fn validate_archive(bytes: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "Tệp Office không hợp lệ.")?;
    if archive.len() > 2000 {
        return Err("Tệp Office có quá nhiều mục.".into());
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|_| "Không đọc được mục Office.")?;
        total = total
            .checked_add(file.size())
            .ok_or("Tệp Office quá lớn.")?;
        if total > MAX_ARCHIVE_BYTES || file.size() > MAX_XML_BYTES {
            return Err("Tệp Office vượt giới hạn giải nén.".into());
        }
    }
    Ok(())
}
fn read_zip_entry(bytes: &[u8], name: &str, budget: &mut u64) -> Result<String, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "Tệp Office không hợp lệ.")?;
    let file = archive
        .by_name(name)
        .map_err(|_| format!("Thiếu {name} trong tài liệu."))?;
    if file.size() > MAX_XML_BYTES {
        return Err("Nội dung Office quá lớn.".into());
    }
    let limit = MAX_XML_BYTES.min(*budget);
    let mut text = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut text)
        .map_err(|_| "Không đọc được văn bản Office.")?;
    if text.len() as u64 > limit {
        return Err("Nội dung Office vượt ngân sách giải nén.".into());
    }
    *budget -= text.len() as u64;
    String::from_utf8(text).map_err(|_| "Văn bản Office không phải UTF-8.".into())
}

fn local_name_eq(e: &quick_xml::events::BytesStart<'_>, tag: &str) -> bool {
    e.local_name().into_inner() == tag
}

fn end_name_eq(e: &quick_xml::events::BytesEnd<'_>, tag: &str) -> bool {
    e.local_name().into_inner() == tag
}

/// Decoded text content of a `BytesText` event (entity-unescaped).
fn text_content(e: &quick_xml::events::BytesText<'_>) -> String {
    quick_xml::escape::unescape(&e.xml10_content())
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| e.xml10_content().into_owned())
}

/// Collect the text of every `<t>` element (Word runs / slide text) joined sensibly.
fn collect_w_t_texts(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut in_t = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if local_name_eq(&e, "t") {
                    in_t = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_t {
                    let s = text_content(&e).trim().to_string();
                    if !s.is_empty() {
                        out.push(s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if end_name_eq(&e, "t") {
                    in_t = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let mut budget = MAX_ARCHIVE_BYTES;
    let xml = read_zip_entry(bytes, "word/document.xml", &mut budget)?;
    Ok(collect_w_t_texts(&xml).join(" "))
}

/// Parse `xl/sharedStrings.xml` into an ordered list of strings.
fn parse_shared_strings(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if local_name_eq(&e, "si") {
                    in_si = true;
                    current.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_si {
                    current.push_str(&text_content(&e));
                }
            }
            Ok(Event::End(e)) => {
                if end_name_eq(&e, "si") {
                    in_si = false;
                    out.push(std::mem::take(&mut current));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Render one worksheet XML as tab-separated rows (capped at [`MAX_SHEET_ROWS`]).
fn render_sheet(xml: &str, shared: &[String]) -> Vec<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rows: Vec<String> = Vec::new();
    let mut cells: Vec<String> = Vec::new();
    let flush_row = |cells: &mut Vec<String>, rows: &mut Vec<String>| {
        if !cells.iter().all(|c| c.trim().is_empty()) {
            rows.push(cells.join("\t"));
        }
        cells.clear();
    };
    let mut in_value = false;
    let mut cell_is_shared = false;
    let mut value = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if local_name_eq(&e, "row") {
                    if rows.len() >= MAX_SHEET_ROWS {
                        break;
                    }
                    if !cells.is_empty() {
                        flush_row(&mut cells, &mut rows);
                    }
                } else if local_name_eq(&e, "c") {
                    cell_is_shared = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .any(|a| a.key.as_ref() == "t" && a.value.as_ref() == "s");
                    value.clear();
                } else if local_name_eq(&e, "v") || local_name_eq(&e, "t") {
                    in_value = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_value {
                    value.push_str(&text_content(&e));
                }
            }
            Ok(Event::End(e)) => {
                if end_name_eq(&e, "v") || end_name_eq(&e, "t") {
                    in_value = false;
                } else if end_name_eq(&e, "c") {
                    let text = if cell_is_shared {
                        value
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| shared.get(i))
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        value.clone()
                    };
                    cells.push(text);
                    cell_is_shared = false;
                    value.clear();
                } else if end_name_eq(&e, "row") {
                    flush_row(&mut cells, &mut rows);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if !cells.is_empty() && rows.len() < MAX_SHEET_ROWS {
        flush_row(&mut cells, &mut rows);
    }
    rows
}

fn list_zip_names(bytes: &[u8]) -> Result<Vec<String>, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Không mở được file Office: {e}"))?;
    let mut names = Vec::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            if let Ok(name) = f.name() {
                names.push(name.into_owned());
            }
        }
    }
    Ok(names)
}

fn extract_xlsx(bytes: &[u8]) -> Result<String, String> {
    let mut budget = MAX_ARCHIVE_BYTES;
    let names = list_zip_names(bytes)?;
    let shared = names
        .iter()
        .any(|n| n == "xl/sharedStrings.xml")
        .then(|| {
            read_zip_entry(bytes, "xl/sharedStrings.xml", &mut budget)
                .map(|x| parse_shared_strings(&x))
        })
        .transpose()?
        .unwrap_or_default();
    let mut sheets: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .cloned()
        .collect();
    sheets.sort();
    if sheets.is_empty() {
        return Err("Không tìm thấy sheet nào trong file Excel.".to_string());
    }
    let mut out = String::new();
    for (i, sheet_name) in sheets.iter().enumerate() {
        let xml = read_zip_entry(bytes, sheet_name, &mut budget)?;
        let rows = render_sheet(&xml, &shared);
        if rows.is_empty() {
            continue;
        }
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&rows.join("\n"));
        if out.chars().count() >= MAX_PREVIEW_CHARS {
            break;
        }
    }
    if out.trim().is_empty() {
        return Err("File Excel không có dữ liệu text.".to_string());
    }
    Ok(out)
}

fn extract_pptx(bytes: &[u8]) -> Result<String, String> {
    let mut budget = MAX_ARCHIVE_BYTES;
    let names = list_zip_names(bytes)?;
    let mut slides: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .cloned()
        .collect();
    slides.sort();
    if slides.is_empty() {
        return Err("Không tìm thấy slide nào trong file PowerPoint.".to_string());
    }
    let mut out = Vec::new();
    for slide_name in &slides {
        let xml = read_zip_entry(bytes, slide_name, &mut budget)?;
        let text = collect_w_t_texts(&xml).join(" ");
        if !text.trim().is_empty() {
            out.push(text);
            if out.iter().map(String::len).sum::<usize>() >= MAX_PREVIEW_CHARS * 4 {
                break;
            }
        }
    }
    if out.is_empty() {
        return Err("File PowerPoint không có dữ liệu text.".to_string());
    }
    Ok(out.join("\n\n"))
}

/// Best-effort RTF strip: drop `\controlword`, keep grouped text.
fn strip_rtf(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => {
                // Control word: letters, optional numeric param, optional trailing space.
                let mut word = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_alphabetic() {
                        word.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() || nc == '-' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if matches!(word.as_str(), "par" | "line" | "tab") {
                    out.push(if word == "tab" { '\t' } else { '\n' });
                } else if word.is_empty() {
                    // Escaped literal char (e.g. \\ \{ \}).
                    if let Some(nc) = chars.next() {
                        if nc != ' ' {
                            out.push(nc);
                        }
                    }
                } else if chars.peek() == Some(&' ') {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    // Collapse 3+ newlines left by stripped groups.
    let mut collapsed = String::with_capacity(out.len());
    let mut blank_run = 0;
    for line in out.split('\n') {
        let t = line.trim();
        if t.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                collapsed.push('\n');
            }
        } else {
            blank_run = 0;
            collapsed.push_str(t);
            collapsed.push('\n');
        }
    }
    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (name, content) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        buf.into_inner()
    }

    const DOCX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body><w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t>World</w:t></w:r></w:p>
<w:p><w:r><w:t>Dòng hai</w:t></w:r></w:p></w:body></w:document>"#;

    const SHARED_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2">
<si><t>Name</t></si><si><t>Tuổi</t></si></sst>"#;

    const SHEET_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>
<row r="2"><c r="A2"><v>42</v></c><c r="B2"><v>7</v></c></row></sheetData></worksheet>"#;

    const SLIDE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
<p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Slide Title</a:t></a:r></a:p>
</p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;

    #[test]
    fn extracts_text_from_a_synthetic_pdf() {
        let content = "BT /F1 12 Tf 20 100 Td (Preview fixture) Tj ET";
        let objects=vec!["<< /Type /Catalog /Pages 2 0 R >>".to_owned(),"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),format!("<< /Length {} >>\nstream\n{content}\nendstream",content.len())];
        let mut pdf = "%PDF-1.4\n".to_owned();
        let mut offsets = vec![0];
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
        }
        let start = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{start}\n%%EOF"
        ));
        assert!(extract_text(pdf.as_bytes(), "pdf")
            .unwrap()
            .contains("Preview fixture"));
    }

    #[test]
    fn rejects_office_expansion_and_oversized_input() {
        let text = "x".repeat(MAX_XML_BYTES as usize + 1);
        let archive = make_zip(&[("word/document.xml", &text)]);
        assert!(extract_text(&archive, "docx").is_err());
        assert!(extract_text(&vec![0; MAX_PREVIEW_BYTES + 1], "txt").is_err());
    }

    #[test]
    fn docx_extracts_w_t_text() {
        let bytes = make_zip(&[
            ("[Content_Types].xml", "<Types/>"),
            ("word/document.xml", DOCX_XML),
        ]);
        let text = extract_text(&bytes, "docx").unwrap();
        assert!(text.contains("Hello"), "got: {text}");
        assert!(text.contains("World"), "got: {text}");
        assert!(text.contains("Dòng hai"), "got: {text}");
    }

    #[test]
    fn xlsx_renders_table_tabs() {
        let bytes = make_zip(&[
            ("[Content_Types].xml", "<Types/>"),
            ("xl/sharedStrings.xml", SHARED_XML),
            ("xl/worksheets/sheet1.xml", SHEET_XML),
        ]);
        let text = extract_text(&bytes, "xlsx").unwrap();
        assert!(text.contains("Name\tTuổi"), "got: {text}");
        assert!(text.contains("42\t7"), "got: {text}");
    }

    #[test]
    fn pptx_extracts_slide_text() {
        let bytes = make_zip(&[
            ("[Content_Types].xml", "<Types/>"),
            ("ppt/slides/slide1.xml", SLIDE_XML),
        ]);
        let text = extract_text(&bytes, "pptx").unwrap();
        assert!(text.contains("Slide Title"), "got: {text}");
    }

    #[test]
    fn txt_csv_passthrough_and_truncation() {
        let text = extract_text("a,b\n1,2".as_bytes(), "csv").unwrap();
        assert_eq!(text, "a,b\n1,2");
        let long = "x".repeat(MAX_PREVIEW_CHARS + 100);
        let clipped = extract_text(long.as_bytes(), "txt").unwrap();
        assert_eq!(clipped.chars().count(), MAX_PREVIEW_CHARS + 1); // + "…"
        assert!(clipped.ends_with('…'));
    }

    #[test]
    fn rtf_strips_control_words() {
        let rtf = r"{\rtf1\ansi Hello\par World\tab!}";
        let text = extract_text(rtf.as_bytes(), "rtf").unwrap();
        assert!(text.contains("Hello"), "got: {text}");
        assert!(text.contains("World"), "got: {text}");
        assert!(!text.contains("rtf1"), "got: {text}");
    }

    #[test]
    fn legacy_ole_errors() {
        for ext in ["doc", "xls", "ppt"] {
            let err = extract_text(b"fake".as_slice(), ext).unwrap_err();
            assert!(err.contains("mở ngoài"), "got: {err}");
        }
    }

    #[test]
    fn unknown_ext_errors() {
        assert!(extract_text(b"fake".as_slice(), "exe").is_err());
    }
}

/// Internal stdin/stdout helper. The GUI launches this in an isolated, timed process.
pub fn helper_entry() -> anyhow::Result<()> {
    use std::io::{Read, Write};
    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_PREVIEW_BYTES as u64 + 1)
        .read_to_end(&mut input)?;
    let result = if input.len() > MAX_PREVIEW_BYTES {
        Err("Tài liệu quá lớn.".to_owned())
    } else {
        extract_text(&input, "pdf").map_err(|_| {
            "Không trích xuất được văn bản PDF. Tệp có thể là ảnh scan hoặc không được hỗ trợ."
                .to_owned()
        })
    };
    std::io::stdout().write_all(&serde_json::to_vec(&result)?)?;
    Ok(())
}
/// PDF parsers may allocate for compressed streams; keep them outside the desktop process.
pub(crate) async fn extract_pdf_isolated(bytes: Vec<u8>) -> anyhow::Result<String> {
    use tokio::io::AsyncWriteExt;
    let executable = std::env::current_exe()?;
    #[cfg(unix)]
    let mut command = {
        // Positional arguments keep filenames out of shell syntax. Limits are inherited by exec.
        let mut c = tokio::process::Command::new("/bin/sh");
        c.arg("-c").arg("ulimit -d 524288 || exit 75; ulimit -t 8 || exit 75; exec \"$1\" --internal-doc-preview").arg("ctdozlite-document").arg(executable);
        c
    };
    #[cfg(not(unix))]
    let mut command = {
        let mut c = tokio::process::Command::new(executable);
        c.arg("--internal-doc-preview");
        c
    };
    let mut child = command
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let output = tokio::time::timeout(std::time::Duration::from_secs(12), async move {
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Missing PDF helper stdin"))?;
        input.write_all(&bytes).await?;
        drop(input);
        Ok::<_, anyhow::Error>(child.wait_with_output().await?)
    })
    .await??;
    anyhow::ensure!(
        output.status.success() && output.stdout.len() <= 128 * 1024,
        "PDF preview failed or exceeded its limits"
    );
    serde_json::from_slice::<Result<String, String>>(&output.stdout)?.map_err(anyhow::Error::msg)
}
