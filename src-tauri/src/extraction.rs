//! External extraction capability boundary. The original stays authoritative.
use crate::office::OfficeProjection;
use std::path::Path;

pub trait SourceExtractor: Send + Sync {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String>;
}

pub fn is_structured_format(format: &str) -> bool {
    matches!(format, "docx" | "pptx" | "pdf" | "eml" | "zip") || crate::photo::is_photo(format)
}

pub fn is_structured_source(path: &Path, format: &str) -> bool {
    is_structured_format(format) || crate::conversation::is_supported_export(path, format)
}

pub struct LocalExtractor;

impl SourceExtractor for LocalExtractor {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
        if matches!(format, "eml" | "zip") {
            crate::containers::isolated_extract(path, format, title)
        } else if format == "pdf" {
            crate::pdf::extract(path, title)
        } else if crate::conversation::is_supported_export(path, format) {
            crate::conversation::extract(path, format, title)
        } else if crate::photo::is_photo(format) {
            crate::photo::extract(path, title)
        } else {
            crate::office::extract(path, format, title)
        }
    }
}
