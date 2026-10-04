import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

export type AcquisitionMethod = "picker" | "drop" | "url";
export type ExtractionState =
  | "text_preserved"
  | "structured_text"
  | "partial_text"
  | "invalid_container"
  | "unsupported"
  | "invalid_utf8"
  | "too_large";

export type CoverageScope =
  | "main_document"
  | "tables"
  | "slide_text"
  | "speaker_notes"
  | "embedded_object";

export type CoverageStatus = "complete" | "partial" | "unsupported" | "failed";

export interface CoveragePart {
  scope: CoverageScope;
  status: CoverageStatus;
  source_location: string;
  detail: string;
}

export interface Acquisition {
  path: string;
  method: AcquisitionMethod;
  received_at: string;
  requested_url?: string | null;
  final_url?: string | null;
  http_status?: number | null;
  content_type?: string | null;
}

export interface SourceInfo {
  schema: number;
  source_id: string;
  page_id: string;
  title: string;
  original_name: string;
  asset: string;
  sha256: string;
  bytes: number;
  format: string;
  current_version_id?: string | null;
  versions_seen?: { source_version_id: string; state: string }[];
  extraction: ExtractionState;
  extraction_detail: string;
  extraction_coverage?: CoveragePart[] | null;
  line_count: number;
  acquisitions: Acquisition[];
  semantic_state:
    "pending" | "processing" | "complete" | "failed" | "unavailable";
  semantic_error: string | null;
  semantic_attempts: number;
  semantic_retry_at: string | null;
  knowledge_pages: KnowledgePageSummary[];
  semantic_decisions: {
    question: string;
    model: string;
    outcome: string;
    probability: number | null;
  }[];
}

export interface SourceSummary {
  source_id: string;
  page_id: string;
  title: string;
  extraction: ExtractionState;
}

export interface SourcePage {
  info: SourceInfo;
  markdown: string;
  body: string;
  knowledge_pages: KnowledgePageSummary[];
}

export interface KnowledgePageSummary {
  page_id: string;
  title: string;
  kind: string;
  path: string;
}

export interface KnowledgePage {
  page_id: string;
  source_id: string;
  title: string;
  kind: string;
  markdown: string;
}

export interface SourceList {
  sources: SourceSummary[];
  next_offset: number | null;
}

export interface PageSearchRequest {
  query: string;
  tags: string[];
  date_from: string | null;
  date_to: string | null;
  format: string | null;
  processing_status: string | null;
  offset: number;
}

export interface SearchMatchLocation {
  record_id: string;
  source_id: string;
  source_version_id: string | null;
  quote: string;
  byte_start: number;
  byte_end: number;
  line_start: number;
  line_end: number;
  offset_basis:
    "preserved_text" | "extracted_office_projection" | "web_visible_text";
  source_location: string | null;
}

export interface PageResult {
  page_id: string;
  source_id: string;
  page_type: "source" | "knowledge";
  title: string;
  kind: string;
  excerpt: string;
  tags: string[];
  format: string;
  event_date: string | null;
  extraction: ExtractionState;
  processing_status:
    "pending" | "processing" | "complete" | "failed" | "unavailable";
  matched_by: "title" | "tag" | "keyword";
  match_location: SearchMatchLocation | null;
}

export interface PageSearchResults {
  pages: PageResult[];
  next_offset: number | null;
  available_tags: string[];
  available_formats: string[];
  available_statuses: string[];
}

/** The reader's application boundary. Desktop paths never come from page HTML. */
export interface GardenApi {
  chooseFile(): Promise<string | null>;
  importSource(path: string, method: AcquisitionMethod): Promise<SourcePage>;
  importUrl(url: string): Promise<SourcePage>;
  listSources(offset: number): Promise<SourceList>;
  searchPages(request: PageSearchRequest): Promise<PageSearchResults>;
  openSource(sourceId: string): Promise<SourcePage>;
  openKnowledgePage(pageId: string): Promise<KnowledgePage>;
  openOriginal(sourceId: string): Promise<void>;
  openOriginalVersion(
    sourceId: string,
    sourceVersionId: string,
    asset: string,
  ): Promise<void>;
  openOriginalAsset(sourceId: string, asset: string): Promise<void>;
  onDrop(handler: (paths: string[]) => void): Promise<() => void>;
}

export const desktopApi: GardenApi = {
  async chooseFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Add a source",
    });
    return typeof selected === "string" ? selected : null;
  },
  importSource: (path, method) => invoke("import_source", { path, method }),
  importUrl: (url) => invoke("import_url", { url }),
  listSources: (offset) => invoke("list_sources", { offset }),
  searchPages: (request) => invoke("search_pages", { request }),
  openSource: (sourceId) => invoke("open_source", { sourceId }),
  openKnowledgePage: (pageId) => invoke("open_knowledge_page", { pageId }),
  openOriginal: (sourceId) => invoke("open_original", { sourceId }),
  openOriginalVersion: (sourceId, sourceVersionId, asset) =>
    invoke("open_original_version", { sourceId, sourceVersionId, asset }),
  openOriginalAsset: (sourceId, asset) =>
    invoke("open_original_asset", { sourceId, asset }),
  async onDrop(handler) {
    return getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "drop") handler(payload.paths);
    });
  },
};
