import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

export type AcquisitionMethod =
  "picker" | "drop" | "url" | "watch" | "repository";
export type ExtractionState =
  | "text_preserved"
  | "structured_text"
  | "partial_text"
  | "invalid_container"
  | "unsupported"
  | "invalid_utf8"
  | "too_large";

export type CoverageScope =
  | "pdf_pages"
  | "main_document"
  | "tables"
  | "slide_text"
  | "speaker_notes"
  | "embedded_object"
  | "conversation_messages"
  | "audio_recording"
  | "image_pixels"
  | "image_metadata"
  | "image_text"
  | "image_interpretation";

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
  path_encoding?: "raw" | "percent_encoded" | null;
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
  repository?: {
    repository_id: string;
    repository_path: string;
    relative_path: string;
  } | null;
  current_version_id?: string | null;
  pending_version_id?: string | null;
  update_status?:
    "pending" | "processing" | "failed" | "incomplete" | "uncertain" | null;
  versions_seen?: {
    source_version_id: string;
    state: string;
    asset?: string;
  }[];
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
  provider_availability?:
    | "network"
    | "quota"
    | "authentication"
    | "model_unavailable"
    | "capability_unavailable"
    | null;
  knowledge_pages: KnowledgePageSummary[];
  semantic_decisions: {
    question: string;
    model: string;
    outcome: string;
    probability: number | null;
  }[];
  audio_processing?: AudioProcessingInfo | null;
  pending_audio_processing?: AudioProcessingInfo | null;
}

export interface AudioProcessingInfo {
  source_version_id: string | null;
  state: "pending" | "processing" | "complete" | "unsupported" | "failed";
  duration_ms: number;
  next_start_ms: number;
  segment_duration_ms: number;
  attempts: number;
  failed_attempts: number;
  processing_interruptions: number;
  retry_at_ms: number | null;
  detail: string;
  segments: {
    segment_id: string;
    start_ms: number;
    end_ms: number;
    text: string;
    confidence: number | null;
    alternatives: string[];
    speaker: string | null;
    speaker_state: string;
    final_result: boolean;
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

export interface RepositoryImportResult {
  repository_id: string;
  sources: Array<{ source_id: string; relative_path: string }>;
  exclusions: Array<{ relative_path: string; reason: string }>;
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
  external_edit_status?: "uncertain" | null;
}

export interface SourceList {
  sources: SourceSummary[];
  next_offset: number | null;
}

export interface PageSearchRequest {
  query: string;
  mode?: "keyword" | "meaning";
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
    | "preserved_text"
    | "extracted_office_projection"
    | "extracted_conversation_projection"
    | "web_visible_text"
    | "audio_transcript"
    | "extracted_image_projection"
    | "extracted_pdf_projection";
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
  matched_by: "title" | "tag" | "keyword" | "meaning";
  match_location: SearchMatchLocation | null;
  meaning_score?: number | null;
}

export interface PageSearchResults {
  pages: PageResult[];
  next_offset: number | null;
  available_tags: string[];
  available_formats: string[];
  available_statuses: string[];
  meaning_search_status?: string;
}

export interface PathExploreRequest {
  start_page_id: string;
  max_hops: number;
  page_size: number;
  relationship_work_budget: number;
  continuation: string | null;
}

export interface ExploredPathStep {
  relationship_id: string;
  from_page_id: string;
  from_title: string;
  to_page_id: string;
  to_title: string;
  kind: string;
  qualifier: string | null;
  traversal_direction: "with_arrow" | "against_arrow";
  evidence_quote: string;
  evidence_location: string;
  evidence_origin: string;
  evidence_qualifier: string | null;
  original_guidance: string;
  source_id: string;
  source_version_id: string;
  original_asset: string;
  source_is_current: boolean;
}

export interface ExploredPath {
  target_page_id: string;
  target_title: string;
  target_kind: string;
  detail_token: string;
  step_count: number;
}

export interface PathDetailsRequest {
  detail_token: string;
  continuation: string | null;
  work_budget: number;
}

export interface PathDetailsResponse {
  target_page_id: string;
  target_title: string;
  steps: ExploredPathStep[];
  complete: boolean;
  next_cursor: string | null;
  work_units: number;
}

export interface PathExploreDiagnostic {
  kind: "missing_endpoint" | "missing_support";
  relationship_id: string;
  endpoint_page_id: string;
}

export interface PathExploreResponse {
  start_page_id: string;
  max_hops: number;
  graph_revision: string;
  paths: ExploredPath[];
  complete: boolean;
  next_cursor: string | null;
  relationships_examined: number;
  diagnostics: PathExploreDiagnostic[];
}

export interface QueryConstraint {
  date_from: string | null;
  date_to: string | null;
  format: string | null;
  processing_status: string | null;
}
export interface QueryHop {
  kind: string;
  direction: "with_arrow" | "against_arrow";
}
export type BooleanExpression =
  | {
      op: "path";
      start_page_id: string;
      hops: QueryHop[];
      endpoint: QueryConstraint;
      intermediate: QueryConstraint;
    }
  | { op: "constraint"; constraint: QueryConstraint }
  | { op: "and"; children: BooleanExpression[] }
  | { op: "or"; children: BooleanExpression[] }
  | { op: "not"; child: BooleanExpression };
export interface BooleanQueryRequest {
  expression: BooleanExpression;
  returned: QueryConstraint;
  continuation: string | null;
  work_budget: number;
  page_size: number;
}
export interface BooleanQueryResult {
  page_id: string;
  source_id: string;
  page_type: string;
  title: string;
  kind: string;
  paths: ExploredPath[];
}
export interface BooleanQueryResponse {
  results: BooleanQueryResult[];
  graph_revision: string;
  next_cursor: string | null;
  complete: boolean;
  work_units: number;
  diagnostics: PathExploreDiagnostic[];
}

export interface TimelineRequest {
  date_meaning: "source_event" | "reasoning_event" | "collection_activity";
  order: "ascending" | "descending";
  query: string;
  tags: string[];
  offset: number;
}

export interface TimelineEntry {
  entry_id: string;
  page_id: string;
  source_id: string;
  title: string;
  kind: string;
  date_label: string;
  date_start: string | null;
  date_end: string | null;
  precision: string;
  date_status: "known" | "approximate" | "unknown" | "conflicting";
  qualifier: string | null;
  support: SearchMatchLocation | null;
}

export interface TimelineResults {
  entries: TimelineEntry[];
  next_offset: number | null;
}

export interface CollectionStatus {
  complete: boolean;
  revision: number;
  diagnostics: Array<{ kind: string; paths: string[]; detail: string }>;
}

/** The reader's application boundary. Desktop paths never come from page HTML. */
export interface GardenApi {
  chooseDirectory?(): Promise<string | null>;
  watchDirectory?(path: string): Promise<WatchedDirectory>;
  listWatchedDirectories?(): Promise<WatchedDirectory[]>;
  openCollectionFile?(path: string): Promise<void>;
  collectionStatus?(): Promise<CollectionStatus>;
  resolvePageLink?(pageId: string, href: string): Promise<string>;
  timeline?(request: TimelineRequest): Promise<TimelineResults>;
  chooseFile(): Promise<string | null>;
  chooseRepositoryDirectory(): Promise<string | null>;
  importSource(path: string, method: AcquisitionMethod): Promise<SourcePage>;
  importRepository(path: string): Promise<RepositoryImportResult>;
  importUrl(url: string): Promise<SourcePage>;
  listUrlAcquisitions(): Promise<UrlAcquisitionStatus[]>;
  listSources(offset: number): Promise<SourceList>;
  searchPages(request: PageSearchRequest): Promise<PageSearchResults>;
  queryPages(request: BooleanQueryRequest): Promise<BooleanQueryResponse>;
  cancelQuery(continuation: string): Promise<void>;
  pathGraphRevision(): Promise<string>;
  explorePaths(request: PathExploreRequest): Promise<PathExploreResponse>;
  explorePathDetails(request: PathDetailsRequest): Promise<PathDetailsResponse>;
  openSource(sourceId: string): Promise<SourcePage>;
  retainedSourceExists?(sourceId: string): Promise<boolean>;
  openKnowledgePage(pageId: string): Promise<KnowledgePage>;
  openOriginal(sourceId: string): Promise<void>;
  previewOriginal?(sourceId: string): Promise<string>;
  openOriginalVersion(
    sourceId: string,
    sourceVersionId: string,
    asset: string,
  ): Promise<void>;
  openOriginalAsset(sourceId: string, asset: string): Promise<void>;
  onDrop(handler: (paths: string[]) => void): Promise<() => void>;
}

export interface WatchedDirectory {
  watch_id: string;
  path: string;
  state: "scanning" | "watching" | "unavailable";
  detail: string | null;
  processed_files: number;
}

export interface UrlAcquisitionStatus {
  url: string;
  attempts: number;
  state: "processing" | "pending" | "restricted" | "failed";
  retry_at: number | null;
  last_error: string;
  previous_source_available: boolean;
}

export const desktopApi: GardenApi = {
  async chooseDirectory() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "Watch directory",
    });
    return typeof selected === "string" ? selected : null;
  },
  watchDirectory: (path) => invoke("watch_directory", { path }),
  listWatchedDirectories: () => invoke("list_watched_directories"),
  openCollectionFile: (path) => invoke("open_collection_file", { path }),
  collectionStatus: () => invoke("collection_status"),
  resolvePageLink: (pageId, href) =>
    invoke("resolve_page_link", { pageId, href }),
  async chooseFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Add a source",
    });
    return typeof selected === "string" ? selected : null;
  },
  async chooseRepositoryDirectory() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "Add a repository",
    });
    return typeof selected === "string" ? selected : null;
  },
  importSource: (path, method) => invoke("import_source", { path, method }),
  importRepository: (path) => invoke("import_repository", { path }),
  importUrl: (url) => invoke("import_url", { url }),
  listUrlAcquisitions: () => invoke("list_url_acquisitions"),
  listSources: (offset) => invoke("list_sources", { offset }),
  searchPages: (request) => invoke("search_pages", { request }),
  queryPages: (request) => invoke("query_pages", { request }),
  cancelQuery: (continuation) => invoke("cancel_query", { continuation }),
  pathGraphRevision: () => invoke("path_graph_revision"),
  explorePaths: (request) => invoke("explore_paths", { request }),
  explorePathDetails: (request) => invoke("explore_path_details", { request }),
  openSource: (sourceId) => invoke("open_source", { sourceId }),
  retainedSourceExists: (sourceId) =>
    invoke("retained_source_exists", { sourceId }),
  openKnowledgePage: (pageId) => invoke("open_knowledge_page", { pageId }),
  openOriginal: (sourceId) => invoke("open_original", { sourceId }),
  previewOriginal: (sourceId) => invoke("preview_original", { sourceId }),
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
