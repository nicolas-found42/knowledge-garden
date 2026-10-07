//! External extraction capability boundary. The original stays authoritative.
use crate::office::OfficeProjection;
use std::path::Path;

pub trait SourceExtractor: Send + Sync {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String>;
}

pub struct LocalExtractor;

impl SourceExtractor for LocalExtractor {
    fn extract(&self, path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
        crate::office::extract(path, format, title)
    }
}
