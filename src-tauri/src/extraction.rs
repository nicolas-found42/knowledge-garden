//! External extraction capability boundary. The original stays authoritative.
use crate::office::OfficeProjection;
use std::path::Path;

pub trait SourceExtractor: Send + Sync {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String>;
}

pub fn is_structured_format(format: &str) -> bool {
    matches!(format, "docx" | "pptx")
        || crate::photo::is_photo(format)
        || crate::conversation::is_conversation(format)
}

pub struct LocalExtractor;

impl SourceExtractor for LocalExtractor {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
        if crate::conversation::is_conversation(format) {
            crate::conversation::extract(path, format, title)
        } else if crate::photo::is_photo(format) {
            crate::photo::extract(path, title)
        } else {
            crate::office::extract(path, format, title)
        }
    }
}
