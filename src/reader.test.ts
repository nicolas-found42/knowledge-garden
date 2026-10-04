import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { mountReader } from "./reader";
import type {
  GardenApi,
  PageSearchRequest,
  PageSearchResults,
  SourcePage,
} from "./api";

const riverside: SourcePage = {
  info: {
    schema: 1,
    source_id: "source-riverside",
    page_id: "page-riverside",
    title: "Riverside notes",
    original_name: "Riverside notes.md",
    asset: "original.md",
    sha256: "a".repeat(64),
    bytes: 72,
    format: "md",
    extraction: "text_preserved",
    extraction_detail:
      "Source text preserved. Semantic fact extraction has not run.",
    line_count: 1,
    semantic_state: "pending",
    semantic_error: null,
    semantic_attempts: 0,
    semantic_retry_at: null,
    knowledge_pages: [],
    semantic_decisions: [],
    acquisitions: [
      {
        path: "/tmp/Riverside notes.md",
        method: "picker",
        received_at: "1790784000000",
      },
    ],
  },
  markdown: "# Riverside notes",
  body: "# Riverside notes\n\n[Open original](original.md)\n\n## Source text · lines 1–1\n\n```text\nObservation V17 took place at Riverside on May 17, 2024.\n```",
  knowledge_pages: [],
};

function testApi(): GardenApi {
  return {
    chooseFile: vi.fn().mockResolvedValue("/tmp/Riverside notes.md"),
    importSource: vi.fn().mockResolvedValue(riverside),
    importUrl: vi.fn().mockResolvedValue(riverside),
    listSources: vi.fn().mockResolvedValue({ sources: [], next_offset: null }),
    searchPages: vi.fn().mockResolvedValue({
      pages: [],
      next_offset: null,
      available_tags: [],
      available_formats: [],
      available_statuses: [],
    } satisfies PageSearchResults),
    openSource: vi.fn().mockResolvedValue(riverside),
    openKnowledgePage: vi.fn(),
    openOriginal: vi.fn().mockResolvedValue(undefined),
    onDrop: vi.fn().mockResolvedValue(() => {}),
  };
}

it("retrieves only the supplied URL, displays its origin, and keeps a failed address available for explicit retry", async () => {
  const api = testApi();
  const page: SourcePage = {
    ...riverside,
    info: {
      ...riverside.info,
      title: "Riverside observation U5",
      format: "html",
      extraction: "structured_text",
      extraction_detail: "Visible page text projected; original retained.",
      acquisitions: [
        {
          path: "http://127.0.0.1:14335/a",
          method: "url",
          requested_url: "http://127.0.0.1:14335/a",
          final_url: "http://127.0.0.1:14335/a",
          http_status: 200,
          content_type: "text/html",
          received_at: "1790784000000",
        },
      ],
    },
    body: "# Riverside observation U5\n\n[destination report](http://127.0.0.1:14335/b)",
  };
  api.importUrl = vi
    .fn()
    .mockRejectedValueOnce(new Error("HTTP 503"))
    .mockResolvedValueOnce(page);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add URL" }));
  const input = screen.getByLabelText("Web address") as HTMLInputElement;
  await user.type(input, "http://127.0.0.1:14335/temporary");
  await user.click(screen.getByRole("button", { name: "Retrieve this page" }));
  await screen.findByText("HTTP 503");
  expect(input.value).toBe("http://127.0.0.1:14335/temporary");
  await user.click(screen.getByRole("button", { name: "Retrieve this page" }));
  expect(api.importUrl).toHaveBeenNthCalledWith(
    2,
    "http://127.0.0.1:14335/temporary",
  );
  expect(
    await screen.findByRole("article", { name: "Riverside observation U5" }),
  ).toBeTruthy();
  expect(screen.getByText("http://127.0.0.1:14335/a")).toBeTruthy();
  expect(
    screen
      .getByRole("link", { name: "destination report" })
      .getAttribute("href"),
  ).toBe("http://127.0.0.1:14335/b");
  expect(screen.getByText("200")).toBeTruthy();
  expect(screen.getByText("text/html")).toBeTruthy();
  expect(
    screen.getByText("Page retained. Linked destinations were not retrieved."),
  ).toBeTruthy();
});

it("searches durable page results with combined filters and restores the exact result view after opening one", async () => {
  const api = testApi();
  const results: PageSearchResults = {
    pages: [
      {
        page_id: "page-v17",
        source_id: "source-riverside",
        page_type: "source",
        title: "Riverside notes",
        kind: "source",
        excerpt: "Observation V17 took place at Riverside.",
        tags: ["fieldwork"],
        format: "txt",
        event_date: "2024-05-17",
        extraction: "text_preserved",
        processing_status: "complete",
        matched_by: "keyword",
        match_location: {
          record_id: "page-v17",
          source_id: "source-riverside",
          source_version_id: "version-riverside-1",
          quote: "Observation V17",
          byte_start: 0,
          byte_end: 15,
          line_start: 1,
          line_end: 1,
          offset_basis: "preserved_text",
          source_location: null,
        },
      },
    ],
    next_offset: null,
    available_tags: ["fieldwork", "riverside"],
    available_formats: ["txt"],
    available_statuses: ["complete", "pending"],
  };
  api.searchPages = vi.fn(async (_request: PageSearchRequest) => results);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("heading", { name: "Search your garden" });
  await user.type(
    screen.getByRole("searchbox", { name: "Words or title" }),
    "Riverside",
  );
  await user.click(screen.getByRole("checkbox", { name: "Tag fieldwork" }));
  fireEvent.change(screen.getByLabelText("From date"), {
    target: { value: "2024-05-01" },
  });
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Format" }),
    "txt",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Processing status" }),
    "complete",
  );
  fireEvent.submit(root.querySelector(".search-form")!);
  const request = (api.searchPages as ReturnType<typeof vi.fn>).mock.calls.at(
    -1,
  )?.[0] as PageSearchRequest;
  expect(request).toEqual({
    query: "Riverside",
    tags: ["fieldwork"],
    date_from: "2024-05-01",
    date_to: null,
    format: "txt",
    processing_status: "complete",
    offset: 0,
  });
  expect(
    await screen.findByText(
      /Evidence · preserved source text · version version-r/,
    ),
  ).toBeTruthy();
  await user.click(
    await screen.findByRole("button", { name: "Riverside notes" }),
  );
  expect(
    await screen.findByRole("article", { name: "Riverside notes" }),
  ).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Back" }));
  expect(
    await screen.findByRole("heading", { name: "Search your garden" }),
  ).toBeTruthy();
  expect(
    screen.getByRole("searchbox", { name: "Words or title" }),
  ).toHaveProperty("value", "Riverside");
  expect(
    screen.getByRole("checkbox", { name: "Tag fieldwork" }),
  ).toHaveProperty("checked", true);
  expect(
    screen.getByRole("button", { name: "Remove #fieldwork filter" }),
  ).toBeTruthy();
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Riverside notes" }),
  );
});

it("shows an explicit empty state and removable search filters", async () => {
  const api = testApi();
  api.searchPages = vi.fn().mockResolvedValue({
    pages: [],
    next_offset: null,
    available_tags: [],
    available_formats: [],
    available_statuses: [],
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  await user.type(
    screen.getByRole("searchbox", { name: "Words or title" }),
    "missing phrase",
  );
  fireEvent.submit(root.querySelector(".search-form")!);
  expect(await screen.findByText(/No matching pages/)).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Remove Words: missing phrase filter" }),
  );
  expect(await screen.findByText(/No matching pages/)).toBeTruthy();
  expect(api.searchPages).toHaveBeenLastCalledWith(
    expect.objectContaining({ query: "", offset: 0 }),
  );
});

let dispose: (() => void) | undefined;
afterEach(() => {
  vi.useRealTimers();
  dispose?.();
  document.body.replaceChildren();
  localStorage.clear();
});

describe("collection reader", () => {
  it("chooses a file, opens its readable page, and opens the retained original", async () => {
    const api = testApi();
    const root = document.createElement("div");
    document.body.append(root);
    dispose = await mountReader(root, api);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Add source" }));
    await screen.findByRole("heading", { name: "Riverside notes" });
    expect(api.importSource).toHaveBeenCalledWith(
      "/tmp/Riverside notes.md",
      "picker",
    );
    expect(
      screen.getByText(
        "Observation V17 took place at Riverside on May 17, 2024.",
      ),
    ).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("article"));
    await user.click(screen.getByRole("link", { name: "Open original" }));
    expect(api.openOriginal).toHaveBeenCalledWith("source-riverside");
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(
      screen
        .getByText(
          "Source text preserved. Semantic fact extraction has not run.",
        )
        .closest("details")?.open,
    ).toBe(false);
  });
});

it("imports dropped files and restores the selected page after reopening the reader", async () => {
  const api = testApi();
  let dropFiles: ((paths: string[]) => void) | undefined;
  api.onDrop = vi.fn(async (handler) => {
    dropFiles = handler;
    return () => {};
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  dropFiles!(["/tmp/Riverside notes.md"]);
  await screen.findByRole("heading", { name: "Riverside notes" });
  expect(api.importSource).toHaveBeenCalledWith(
    "/tmp/Riverside notes.md",
    "drop",
  );
  dispose();
  dispose = await mountReader(root, api);
  expect(api.openSource).toHaveBeenCalledWith("source-riverside");
  expect(
    screen.getByText(
      "Observation V17 took place at Riverside on May 17, 2024.",
    ),
  ).toBeTruthy();
});

it("shows background semantic completion on the active source without closing its details", async () => {
  const api = testApi();
  api.importSource = vi.fn().mockResolvedValue(riverside);
  api.openSource = vi.fn().mockResolvedValue({
    ...riverside,
    info: {
      ...riverside.info,
      semantic_state: "complete",
      knowledge_pages: [
        {
          page_id: "page-v17",
          title: "Observation V17",
          kind: "event",
          path: "pages/page-v17.md",
        },
      ],
    },
    body: `${riverside.body}\n\n[Observation V17](../pages/page-v17.md)`,
    knowledge_pages: [
      {
        page_id: "page-v17",
        title: "Observation V17",
        kind: "event",
        path: "pages/page-v17.md",
      },
    ],
  });
  vi.useFakeTimers();
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("heading", { name: "Riverside notes" });
  await user.click(screen.getByText("Source information"));
  expect(screen.getByText("Waiting for semantic processing")).toBeTruthy();

  await vi.advanceTimersByTimeAsync(3000);

  expect(await screen.findByText("Ready")).toBeTruthy();
  expect(screen.getByRole("link", { name: "Observation V17" })).toBeTruthy();
  expect(screen.getByText("Source information").closest("details")?.open).toBe(
    true,
  );
});

it("shows permanent semantic failure separately and stops polling that source", async () => {
  const failed: SourcePage = {
    ...riverside,
    info: {
      ...riverside.info,
      semantic_state: "failed",
      semantic_error: "Jev returned HTTP 400; semantic processing failed.",
    },
  };
  const api = testApi();
  api.importSource = vi.fn().mockResolvedValue(failed);
  api.openSource = vi.fn().mockResolvedValue(failed);
  vi.useFakeTimers();
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("heading", { name: "Riverside notes" });
  await user.click(screen.getByText("Source information"));
  expect(screen.getByText("Processing failed")).toBeTruthy();
  expect(screen.getByText(failed.info.semantic_error!)).toBeTruthy();

  await vi.advanceTimersByTimeAsync(6000);

  expect(api.openSource).not.toHaveBeenCalled();
  expect(screen.getByRole("article", { name: "Riverside notes" })).toBeTruthy();
});

it("exposes unsupported coverage and provenance on demand with keyboard access to the original", async () => {
  const api = testApi();
  const unsupported: SourcePage = {
    ...riverside,
    info: {
      ...riverside.info,
      title: "Unsupported input",
      extraction: "unsupported",
      extraction_detail:
        "This format is not supported by the text importer. The original is retained.",
    },
    body: "# Unsupported input\n\n[Open original](original.md)\n\nText is unavailable. The original is retained.",
  };
  api.importSource = vi.fn().mockResolvedValue(unsupported);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("heading", { name: "Unsupported input" });
  expect(screen.getByRole("status").textContent).toContain(
    "The original is retained",
  );
  await user.tab();
  expect(document.activeElement).toBe(
    screen.getByRole("link", { name: "Open original" }),
  );
  await user.keyboard("{Enter}");
  expect(api.openOriginal).toHaveBeenCalledWith("source-riverside");
  await user.click(screen.getByText("Source information"));
  expect(screen.getByText("Unsupported format").closest("details")?.open).toBe(
    true,
  );
});

it("keeps the current page readable after an import fails and allows another import", async () => {
  const api = testApi();
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("heading", { name: "Riverside notes" });
  api.importSource = vi
    .fn()
    .mockRejectedValueOnce(new Error("The file is unavailable."))
    .mockResolvedValue(riverside);
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain(
      "The file is unavailable.",
    ),
  );
  expect(screen.getByRole("heading", { name: "Riverside notes" })).toBeTruthy();
  expect(
    screen.getByRole<HTMLButtonElement>("button", { name: "Add source" })
      .disabled,
  ).toBe(false);
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toBe("Source added."),
  );
});

it("navigates collection pages using bounded source lists and prevents source HTML from executing", async () => {
  const api = testApi();
  api.listSources = vi.fn().mockResolvedValue({
    sources: [
      {
        source_id: riverside.info.source_id,
        page_id: riverside.info.page_id,
        title: riverside.info.title,
        extraction: "text_preserved",
      },
    ],
    next_offset: null,
  });
  api.openSource = vi.fn().mockResolvedValue({
    ...riverside,
    body:
      riverside.body +
      '\n<script>window.compromised=true</script><img src="https://example.com/pixel" onerror="alert(1)">',
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Sources" }));
  await user.click(
    await screen.findByRole("button", { name: "Riverside notes" }),
  );
  await screen.findByRole("heading", { name: "Riverside notes" });
  expect(api.listSources).toHaveBeenCalledWith(0);
  expect(api.openSource).toHaveBeenCalledWith(riverside.info.source_id);
  expect(screen.getByRole("article").querySelector("script, img")).toBeNull();
  expect(screen.getByRole("navigation", { hidden: true }).hidden).toBe(true);
});

it("opens linked knowledge pages, returns to their source, and opens the retained original", async () => {
  const digest = "b".repeat(64);
  const source: SourcePage = {
    ...riverside,
    info: { ...riverside.info, source_id: `source-${digest}` },
    body: `# V17 report\n\n[Observation V17](pages/page-v17.md)\n\n[Open original](original.md)`,
  };
  const knowledge = {
    page_id: "page-v17",
    source_id: source.info.source_id,
    title: "Observation V17",
    kind: "event",
    markdown: `---\npage_id: page-v17\n---\n\n# Observation V17\n\n12 visits.\n\n[Source page](../sources/${digest}/index.md) · [Retained original](../sources/${digest}/original.txt)`,
  };
  const api = testApi();
  api.importSource = vi.fn().mockResolvedValue(source);
  api.openKnowledgePage = vi.fn().mockResolvedValue(knowledge);
  api.openSource = vi.fn().mockResolvedValue(source);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await user.click(
    await screen.findByRole("link", { name: "Observation V17" }),
  );
  expect(
    await screen.findByRole("article", { name: "Observation V17" }),
  ).toBeTruthy();
  expect(api.openKnowledgePage).toHaveBeenCalledWith("page-v17");
  await user.click(screen.getByRole("link", { name: "Source page" }));
  expect(api.openSource).toHaveBeenCalledWith(source.info.source_id);
  await user.click(screen.getByRole("button", { name: "Back" }));
  expect(
    await screen.findByRole("article", { name: "Observation V17" }),
  ).toBeTruthy();
  expect(document.activeElement).toBe(
    screen.getByRole("link", { name: "Source page" }),
  );
  await user.click(screen.getByRole("link", { name: "Retained original" }));
  expect(api.openOriginal).toHaveBeenCalledWith(source.info.source_id);
});
