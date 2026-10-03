import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

export type AcquisitionMethod = "picker" | "drop";
export type ExtractionState =
  "text_preserved" | "unsupported" | "invalid_utf8" | "too_large";

export interface Acquisition {
  path: string;
  method: AcquisitionMethod;
  received_at: string;
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
  extraction: ExtractionState;
  extraction_detail: string;
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

/** The reader's application boundary. Desktop paths never come from page HTML. */
export interface GardenApi {
  chooseFile(): Promise<string | null>;
  importSource(path: string, method: AcquisitionMethod): Promise<SourcePage>;
  listSources(offset: number): Promise<SourceList>;
  openSource(sourceId: string): Promise<SourcePage>;
  openKnowledgePage(pageId: string): Promise<KnowledgePage>;
  openOriginal(sourceId: string): Promise<void>;
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
  listSources: (offset) => invoke("list_sources", { offset }),
  openSource: (sourceId) => invoke("open_source", { sourceId }),
  openKnowledgePage: (pageId) => invoke("open_knowledge_page", { pageId }),
  openOriginal: (sourceId) => invoke("open_original", { sourceId }),
  async onDrop(handler) {
    return getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "drop") handler(payload.paths);
    });
  },
};
