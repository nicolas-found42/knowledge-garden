mod office {
    #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum CoverageScope {
        MainDocument,
    }
    #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum CoverageStatus {
        Complete,
        Partial,
        Unsupported,
        Failed,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CoveragePart {
        pub scope: CoverageScope,
        pub status: CoverageStatus,
        pub source_location: Option<String>,
        pub detail: String,
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct OfficeProjection {
        pub markdown: String,
        pub semantic_text: String,
        pub line_count: usize,
        pub coverage: Vec<CoveragePart>,
        pub partial: bool,
        pub detail: String,
    }
}
#[path = "../../src/containers.rs"]
#[allow(dead_code)]
mod containers;
fn main() {
    containers::helper();
}
