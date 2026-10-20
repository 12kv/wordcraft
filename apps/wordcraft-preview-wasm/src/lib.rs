//! Headless conversion for browser workers; no UI, GPU or operating-system fonts.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const PLACEHOLDER: &str = "Baimiao Preview Placeholder";

fn read(bytes: &[u8]) -> Result<wordcraft_doc::Document, String> {
    // Package reading bounds decompressed parts, total bytes and XML complexity.
    wordcraft_fonts::FontDb::global().set_system_fallback(false);
    wordcraft_docx::read(bytes).map_err(|e| format!("INVALID_DOCX: {e}"))
}

fn ignored(ch: char) -> bool {
    ch.is_control()
        || ch.is_whitespace()
        || matches!(ch as u32, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2069 | 0xFE00..=0xFE0F | 0xFEFF | 0xFFFC | 0xE0100..=0xE01EF)
}

fn missing(document: &wordcraft_doc::Document) -> (u32, Vec<u32>) {
    let db = wordcraft_fonts::FontDb::global();
    let base = db.face(wordcraft_fonts::DEFAULT_FAMILY, "Regular");
    let mut coverage = std::collections::HashMap::new();
    let mut codes = std::collections::BTreeSet::new();
    let mut count = 0u32;
    let stories = std::iter::once(wordcraft_doc::StoryRef::Body).chain(document.parts.keys().map(|id| wordcraft_doc::StoryRef::Part(*id)));
    for story in stories {
        for ch in document.plain_text(story).chars().filter(|ch| !ignored(*ch)) {
            let covered =
                *coverage.entry(ch).or_insert_with(|| base.covers(ch) || db.fallback_for(ch, base.id()).is_some_and(|f| f.family != PLACEHOLDER));
            if !covered {
                count = count.saturating_add(1);
                codes.insert(ch as u32);
            }
        }
    }
    (count, codes.into_iter().collect())
}

/// Scan before optional font downloads; never upload document text.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn missing_docx_codepoints(bytes: &[u8]) -> Result<Vec<u32>, String> {
    Ok(missing(&read(bytes)?).1)
}

/// Character inventory is local-only and also preserves the old font fallback choices.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn docx_codepoints(bytes: &[u8]) -> Result<Vec<u32>, String> {
    let doc = read(bytes)?;
    let mut codes = std::collections::BTreeSet::new();
    for story in std::iter::once(wordcraft_doc::StoryRef::Body).chain(doc.parts.keys().map(|id| wordcraft_doc::StoryRef::Part(*id))) {
        codes.extend(doc.plain_text(story).chars().filter(|ch| !ignored(*ch)).map(|ch| ch as u32));
    }
    for list in &doc.numbering.abstracts {
        for level in &list.levels {
            codes.extend(level.text.chars().filter(|ch| !ignored(*ch)).map(|ch| ch as u32));
        }
    }
    Ok(codes.into_iter().collect())
}

/// Effective run fonts include inherited paragraph/character styles and secondary stories.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn docx_font_names(bytes: &[u8]) -> Result<String, String> {
    let doc = read(bytes)?;
    let mut names = std::collections::BTreeSet::new();
    let mut add = |props: &wordcraft_doc::props::CharProps, style: Option<&str>| {
        let resolved = doc.styles.resolve_char(style, props);
        names.insert(resolved.font);
        names.insert(resolved.font_cs);
        if let Some(font) = resolved.font_ea {
            names.insert(font);
        }
    };
    for story in std::iter::once(wordcraft_doc::StoryRef::Body).chain(doc.parts.keys().map(|id| wordcraft_doc::StoryRef::Part(*id))) {
        for path in doc.para_paths(story) {
            if let Some(para) = doc.para(story, &path) {
                add(&para.mark, para.props.style.as_deref());
                for run in &para.runs {
                    add(&run.props, para.props.style.as_deref());
                }
            }
        }
    }
    // Table conditional styles and list labels can specify fonts independently of runs.
    for style in &doc.styles.styles {
        if let Some(table) = &style.table {
            for props in
                [&style.chr, &table.header_chr, &table.band_chr, &table.first_col_chr, &table.total_chr, &table.last_col_chr, &table.col_band_chr]
            {
                add(props, None);
            }
        }
    }
    for list in &doc.numbering.abstracts {
        for level in &list.levels {
            add(&level.chr, None);
        }
    }
    for num in &doc.numbering.nums {
        for (_, level) in &num.level_overrides {
            add(&level.chr, None);
        }
    }
    Ok(names.into_iter().collect::<Vec<_>>().join("\n"))
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn register_preview_font(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 24 * 1024 * 1024 {
        return Err("FONT_TOO_LARGE".into());
    }
    if wordcraft_fonts::FontDb::global().add_font(bytes.to_vec()) == 0 {
        return Err("INVALID_OR_DUPLICATE_FONT".into());
    }
    Ok(())
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct PreviewOutput {
    pdf: Vec<u8>,
    pub missing_glyphs: u32,
    codes: Vec<u32>,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PreviewOutput {
    pub fn take_pdf(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pdf)
    }
    pub fn missing_codepoints(&self) -> Vec<u32> {
        self.codes.clone()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn convert_docx_preview(bytes: &[u8]) -> Result<PreviewOutput, String> {
    let document = read(bytes)?;
    let (missing_glyphs, codes) = missing(&document);
    let db = wordcraft_fonts::FontDb::global();
    if !db.has_family(PLACEHOLDER)
        && let Some(font) = wordcraft_fonts::CRAFT_FONTS.iter().find(|font| font.family == PLACEHOLDER)
    {
        db.add_font(font.bytes.to_vec());
    }
    if missing_glyphs > 0 && !db.has_family(PLACEHOLDER) {
        return Err("MISSING_PLACEHOLDER_FONT".into());
    }
    let pdf = wordcraft_pdf::export(&document, &wordcraft_pdf::PdfOptions::default()).map_err(|e| format!("CONVERSION_FAILED: {e}"))?;
    Ok(PreviewOutput { pdf, missing_glyphs, codes })
}

/// Legacy conversion API; callers needing completeness status must use convert_docx_preview.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn convert_docx_to_pdf(bytes: &[u8]) -> Result<Vec<u8>, String> {
    Ok(convert_docx_preview(bytes)?.pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_font_names_include_inherited_and_east_asian_names() {
        let mut doc = wordcraft_doc::Document::from_text("ASCII only");
        doc.styles.default_chr.font = Some("Inter".into());
        doc.styles.default_chr.font_ea = Some("Noto Sans CJK SC".into());
        let bytes = wordcraft_docx::write(&doc).unwrap();
        let names = docx_font_names(&bytes).unwrap();
        assert!(names.lines().any(|name| name == "Inter"));
        assert!(names.lines().any(|name| name == "Noto Sans CJK SC"));
    }

    #[test]
    fn malformed_package_is_an_error() {
        assert!(convert_docx_to_pdf(b"not a DOCX").unwrap_err().starts_with("INVALID_DOCX:"));
    }

    #[test]
    fn large_invalid_input_reaches_package_validation() {
        assert!(convert_docx_to_pdf(&vec![0; 21 * 1024 * 1024]).unwrap_err().starts_with("INVALID_DOCX:"));
    }

    #[test]
    fn valid_document_returns_pdf_bytes() {
        let doc = wordcraft_doc::Document::from_text("Browser conversion text");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        assert!(convert_docx_to_pdf(&bytes).unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    fn mathematical_symbols_convert_without_system_fonts() {
        if !wordcraft_fonts::CRAFT_FONTS.iter().any(|font| font.scripts.contains(&"Zmth")) {
            assert!(std::env::var_os("CRAFT_FONTS_REQUIRED").is_none(), "required math font missing");
            return;
        }
        let doc = wordcraft_doc::Document::from_text("u∗ Y∗ ∝ ≫ ⊤ ⋅ ⏟ ⟹ ⟺ ∑ ∫ √ ∞ ≤ ≥");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        assert!(convert_docx_to_pdf(&bytes).unwrap().starts_with(b"%PDF-"));
    }

    fn has_placeholder_fixture() -> bool {
        let present = wordcraft_fonts::CRAFT_FONTS.iter().any(|font| font.family == PLACEHOLDER);
        assert!(present || std::env::var_os("CRAFT_FONTS_REQUIRED").is_none(), "required placeholder font missing");
        present
    }

    #[test]
    fn missing_characters_remain_detectable_after_placeholder_registration() {
        if !has_placeholder_fixture() {
            return;
        }
        let doc = wordcraft_doc::Document::from_text("before \u{20000} \u{20000} \u{F0000} after");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        for _ in 0..2 {
            let result = convert_docx_preview(&bytes).unwrap();
            assert_eq!(result.missing_glyphs, 3);
            assert_eq!(result.codes, vec![0x20000, 0xF0000]);
            assert_eq!(missing_docx_codepoints(&bytes).unwrap(), vec![0x20000, 0xF0000]);
        }
        assert_eq!(read(&bytes).unwrap().plain_text(wordcraft_doc::StoryRef::Body).trim(), "before \u{20000} \u{20000} \u{F0000} after");
    }

    #[test]
    fn unrepresentable_text_returns_pdf_with_explicit_warning() {
        if !has_placeholder_fixture() {
            return;
        }
        let doc = wordcraft_doc::Document::from_text("\u{10FFFF}");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        let result = convert_docx_preview(&bytes).unwrap();
        assert!(result.pdf.starts_with(b"%PDF-"));
        assert_eq!(result.missing_glyphs, 1);
        assert_eq!(result.codes, vec![0x10FFFF]);
        let db = wordcraft_fonts::FontDb::global();
        let face = db.face(PLACEHOLDER, "Regular");
        assert!(face.covers('\u{10FFFF}'));
        assert_ne!(face.glyph_for('\u{10FFFF}'), 0);
    }
}
