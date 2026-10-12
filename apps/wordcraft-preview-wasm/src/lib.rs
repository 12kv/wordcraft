//! Headless conversion for browser workers; no UI, GPU or operating-system fonts.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;

/// Converts a DOCX package to PDF bytes. Errors are returned to the worker, never a dialog.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn convert_docx_to_pdf(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("INPUT_TOO_LARGE: DOCX exceeds 20 MiB".into());
    }
    wordcraft_fonts::FontDb::global().set_system_fallback(false);
    let document = wordcraft_docx::read(bytes).map_err(|e| format!("INVALID_DOCX: {e}"))?;
    // Text that our embedded fonts cannot represent must offer cloud retry rather than
    // quietly produce boxes. This is a coverage gate, not a claim of Word layout fidelity.
    let db = wordcraft_fonts::FontDb::global();
    let base = db.face(wordcraft_fonts::DEFAULT_FAMILY, "Regular");
    let mut checked = std::collections::HashSet::new();
    let stories = std::iter::once(wordcraft_doc::StoryRef::Body).chain(document.parts.keys().map(|id| wordcraft_doc::StoryRef::Part(*id)));
    for story in stories {
        for ch in document.plain_text(story).chars() {
            if ch.is_control()
                || ch.is_whitespace()
                || matches!(ch as u32, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2069 | 0xFE00..=0xFE0F | 0xFEFF | 0xFFFC | 0xE0100..=0xE01EF)
                || !checked.insert(ch)
            {
                continue;
            }
            if !base.covers(ch) && db.fallback_for(ch, base.id()).is_none() {
                return Err(format!("UNSUPPORTED_TEXT: no embedded glyph for U+{:04X}", ch as u32));
            }
        }
    }
    wordcraft_pdf::export(&document, &wordcraft_pdf::PdfOptions::default()).map_err(|e| format!("CONVERSION_FAILED: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_package_is_an_error() {
        assert!(convert_docx_to_pdf(b"not a DOCX").unwrap_err().starts_with("INVALID_DOCX:"));
    }

    #[test]
    fn oversized_input_is_rejected_before_parsing() {
        assert!(convert_docx_to_pdf(&vec![0; MAX_INPUT_BYTES + 1]).unwrap_err().starts_with("INPUT_TOO_LARGE:"));
    }

    #[test]
    fn valid_document_returns_pdf_bytes() {
        let doc = wordcraft_doc::Document::from_text("Browser conversion text");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        assert!(convert_docx_to_pdf(&bytes).unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    fn unrepresentable_text_is_rejected_instead_of_becoming_a_box() {
        let doc = wordcraft_doc::Document::from_text("\u{10FFFF}");
        let bytes = wordcraft_docx::write(&doc).unwrap();
        assert!(convert_docx_to_pdf(&bytes).unwrap_err().starts_with("UNSUPPORTED_TEXT:"));
    }
}
