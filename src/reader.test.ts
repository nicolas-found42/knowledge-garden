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
    listUrlAcquisitions: vi.fn().mockResolvedValue([]),
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
    openOriginalVersion: vi.fn().mockResolvedValue(undefined),
    openOriginalAsset: vi.fn().mockResolvedValue(undefined),
    onDrop: vi.fn().mockResolvedValue(() => {}),
  };
}

it("loads a photo preview only on request and keeps its region guidance beside the image", async () => {
  const api = testApi();
  const preview = vi.fn().mockResolvedValue("data:image/jpeg;base64,/9j/");
  Object.assign(api, { previewOriginal: preview });
  api.importSource = vi.fn().mockResolvedValue({
    ...riverside,
    info: { ...riverside.info, format: "png", title: "Photo evidence" },
    body: "# Photo evidence\n\nOCR region 1 · normalized bottom-left x=0.2,y=0.3,width=0.4,height=0.1 · RIVER SURVEY",
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  await userEvent
    .setup()
    .click(screen.getByRole("button", { name: "Add source" }));
  expect(preview).not.toHaveBeenCalled();
  await userEvent
    .setup()
    .click(screen.getByRole("button", { name: "Show image preview" }));
  expect(preview).toHaveBeenCalledWith("source-riverside");
  const image = await screen.findByRole("img", {
    name: "Retained photo preview: Photo evidence",
  });
  expect(image.getAttribute("src")).toBe("data:image/jpeg;base64,/9j/");
  expect(screen.getByRole("article").textContent).toContain(
    "normalized bottom-left",
  );
  expect(screen.getByText(/whole-image fallback/)).toBeTruthy();
});

it("keeps prior content readable and shows an incomplete update prominently", async () => {
  const api = testApi();
  api.importSource = vi.fn().mockResolvedValue({
    ...riverside,
    info: {
      ...riverside.info,
      semantic_state: "complete",
      update_status: "pending",
    },
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  await userEvent
    .setup()
    .click(screen.getByRole("button", { name: "Add source" }));
  expect(
    await screen.findByText(
      "Update pending · Showing the last successful version.",
    ),
  ).toBeTruthy();
  expect(screen.getByRole("article").textContent).toContain("Observation V17");
  expect(screen.getByRole("link", { name: "Open original" })).toBeTruthy();
});

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
  api.listUrlAcquisitions = vi.fn().mockResolvedValue([
    {
      url: "http://127.0.0.1:14335/temporary",
      attempts: 1,
      state: "pending",
      retry_at: Date.now() + 60_000,
      last_error: "HTTP 503",
      previous_source_available: true,
    },
  ]);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add URL" }));
  const input = screen.getByLabelText("Web address") as HTMLInputElement;
  await user.type(input, "http://127.0.0.1:14335/temporary");
  await user.click(screen.getByRole("button", { name: "Retrieve this page" }));
  await screen.findByText("HTTP 503");
  expect(await screen.findByText(/pending; 1 attempt/)).toBeTruthy();
  expect(
    screen.getByText(/Previously retained source material remains available/),
  ).toBeTruthy();
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
  api.openSource = vi.fn().mockResolvedValue({
    ...riverside,
    info: {
      ...riverside.info,
      semantic_state: "complete",
      current_version_id: "version-riverside-1",
      versions_seen: [
        { source_version_id: "version-riverside-1", state: "superseded" },
      ],
    },
  });
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
    mode: "keyword",
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
  await user.click(screen.getByText("Source information"));
  expect(
    screen.getByText(
      "Superseded source version retained as history. Its original remains available.",
    ),
  ).toBeTruthy();
  expect(screen.getByRole("link", { name: "Open original" })).toBeTruthy();
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

it("opens a conversation match with its neighboring messages and restores the result focus", async () => {
  const api = testApi();
  const decision = "Decision: use plan Alder; abandon the Birch suggestion.";
  const locator =
    "CONVERSATION session=plan; message=user-2; order=5; role=user; author=owner; date=unknown; channel=message";
  api.searchPages = vi.fn().mockResolvedValue({
    pages: [
      {
        page_id: "page-conversation",
        source_id: "source-conversation",
        page_type: "source",
        title: "Deployment conversation",
        kind: "source",
        excerpt: decision,
        tags: [],
        format: "jsonl",
        event_date: null,
        extraction: "structured_text",
        processing_status: "complete",
        matched_by: "keyword",
        match_location: {
          record_id: "message-user-2",
          source_id: "source-conversation",
          source_version_id: "version-conversation",
          quote: decision,
          byte_start: 0,
          byte_end: decision.length,
          line_start: 6,
          line_end: 6,
          offset_basis: "extracted_conversation_projection",
          source_location: locator,
        },
      },
    ],
    next_offset: null,
    available_tags: [],
    available_formats: ["jsonl"],
    available_statuses: ["complete"],
  } satisfies PageSearchResults);
  api.openSource = vi.fn().mockResolvedValue({
    ...riverside,
    info: {
      ...riverside.info,
      source_id: "source-conversation",
      page_id: "page-conversation",
      title: "Deployment conversation",
      format: "jsonl",
      semantic_state: "complete",
    },
    body: `# Deployment conversation\n\nSupplied conversation records; not independent evidence that a deployment occurred.\n\n\`\`\`text\n[message=assistant-1; role=assistant; date=unknown] I suggest plan Birch instead.\n[message=output-check-1; role=tool; date=unknown] Check failed: plan Birch has no rollback.\n[${locator}] ${decision}\n[message=reason-1; role=reasoning; date=unknown] Birch lacks rollback; Alder retains one.\n\`\`\``,
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  await user.type(
    await screen.findByRole("searchbox", { name: "Words or title" }),
    "Alder",
  );
  fireEvent.submit(root.querySelector(".search-form")!);
  expect(
    await screen.findByText(/Evidence · extracted conversation messages/),
  ).toBeTruthy();
  expect(screen.getByText(new RegExp("message=user-2"))).toBeTruthy();
  await user.click(
    await screen.findByRole("button", { name: "Deployment conversation" }),
  );
  const article = await screen.findByRole("article", {
    name: "Deployment conversation",
  });
  expect(article.textContent).toContain("I suggest plan Birch instead.");
  expect(article.textContent).toContain(
    "Check failed: plan Birch has no rollback.",
  );
  expect(article.textContent).toContain(decision);
  expect(article.textContent).toContain(
    "role=user; author=owner; date=unknown",
  );
  expect(article.textContent).toContain(
    "not independent evidence that a deployment occurred",
  );
  expect(article.querySelector("mark")?.textContent).toBe(decision);
  await user.click(screen.getByRole("button", { name: "Back" }));
  expect(
    await screen.findByRole("searchbox", { name: "Words or title" }),
  ).toHaveProperty("value", "Alder");
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Deployment conversation" }),
  );
});

it("sends remembered-meaning searches through shared page results and restores keyword mode from its filter", async () => {
  const api = testApi();
  api.searchPages = vi.fn().mockResolvedValue({
    pages: [],
    next_offset: null,
    available_tags: [],
    available_formats: [],
    available_statuses: [],
    meaning_search_status: "missing_assets: local bundle missing",
  } satisfies PageSearchResults);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("heading", { name: "Search your garden" });
  await user.type(
    screen.getByRole("searchbox", { name: "Words or title" }),
    "river notes",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Search mode" }),
    "meaning",
  );
  fireEvent.submit(root.querySelector(".search-form")!);
  await waitFor(() => {
    expect(api.searchPages).toHaveBeenLastCalledWith(
      expect.objectContaining({ query: "river notes", mode: "meaning" }),
    );
  });
  expect(await screen.findByText(/No matching pages/)).toBeTruthy();
  expect(
    screen.getByText(
      /Meaning search is unavailable: missing_assets: local bundle missing/,
    ),
  ).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Remove Meaning search filter" }),
  );
  await waitFor(() => {
    expect(api.searchPages).toHaveBeenLastCalledWith(
      expect.objectContaining({ query: "river notes", mode: "keyword" }),
    );
  });
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
it("keeps the latest search visible when an older response arrives later", async () => {
  const api = testApi();
  const results: PageSearchResults = {
    pages: [],
    next_offset: null,
    available_tags: [],
    available_formats: [],
    available_statuses: [],
  };
  let releaseOlder!: (value: PageSearchResults) => void;
  api.searchPages = vi
    .fn()
    .mockResolvedValueOnce(results)
    .mockImplementationOnce(
      () =>
        new Promise<PageSearchResults>((resolve) => {
          releaseOlder = resolve;
        }),
    )
    .mockResolvedValueOnce(results);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  const query = await screen.findByRole("searchbox", {
    name: "Words or title",
  });
  await user.type(query, "older request");
  fireEvent.submit(root.querySelector(".search-form")!);
  await user.clear(query);
  await user.type(query, "latest request");
  fireEvent.submit(root.querySelector(".search-form")!);
  await screen.findByRole("button", {
    name: "Remove Words: latest request filter",
  });
  releaseOlder(results);
  await Promise.resolve();
  await Promise.resolve();
  expect(
    screen.getByRole("searchbox", { name: "Words or title" }),
  ).toHaveProperty("value", "latest request");
  expect(
    screen.getByRole("button", { name: "Remove Words: latest request filter" }),
  ).toBeTruthy();
});
it("keeps an opened source visible when a pending search response arrives", async () => {
  const api = testApi();
  let releaseSearch!: (value: PageSearchResults) => void;
  api.searchPages = vi.fn(
    () =>
      new Promise<PageSearchResults>((resolve) => {
        releaseSearch = resolve;
      }),
  );
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Search" }));
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("article", { name: "Riverside notes" });
  releaseSearch({
    pages: [],
    next_offset: null,
    available_tags: [],
    available_formats: [],
    available_statuses: [],
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(screen.getByRole("article", { name: "Riverside notes" })).toBeTruthy();
});
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

  it("opens the retained recording for a transcript timestamp and discloses manual seek fallback", async () => {
    const api = testApi();
    const digest = "b".repeat(64);
    const audioPage: SourcePage = {
      ...riverside,
      info: {
        ...riverside.info,
        source_id: `source-${digest}`,
        format: "m4a",
        asset: "original.m4a",
      },
      body: `# Visit recording\n\n[Play retained original](original.m4a)\n\n[1,000–2,000 ms · confidence 0.51 · speaker unidentified](?audio_seek=source-${digest}&at_ms=1000)`,
      markdown: "# Visit recording",
    };
    vi.mocked(api.importSource).mockResolvedValue(audioPage);
    const root = document.createElement("div");
    document.body.append(root);
    dispose = await mountReader(root, api);
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Add source" }));
    const timestamp = await screen.findByRole("link", {
      name: /1,000–2,000 ms/,
    });
    await userEvent.setup().click(timestamp);
    await waitFor(() =>
      expect(api.openOriginal).toHaveBeenCalledWith(`source-${digest}`),
    );
    expect(screen.getByText(/Seek to 0:01 in your audio player/)).toBeTruthy();
    expect(
      screen.getByText(/precise seeking is unavailable in this reader/),
    ).toBeTruthy();
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
  const oldVersion = "c".repeat(64);
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
    markdown: `---\npage_id: page-v17\n---\n\n# Observation V17\n\n12 visits.\n\n[Source page](../sources/${digest}/index.md) · [Current original](../sources/${digest}/original.txt) · [Retained original](../sources/${digest}/versions/${oldVersion}/original.txt)`,
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
  await user.click(screen.getByRole("link", { name: "Current original" }));
  expect(api.openOriginalAsset).toHaveBeenCalledWith(
    source.info.source_id,
    "original.txt",
  );
  await user.click(screen.getByRole("link", { name: "Retained original" }));
  expect(api.openOriginalVersion).toHaveBeenCalledWith(
    source.info.source_id,
    oldVersion,
    "original.txt",
  );
});

it("shows Office projection offsets, original locators, and fallback guidance in the reader", async () => {
  const digest = "d".repeat(64);
  const source: SourcePage = {
    ...riverside,
    info: {
      ...riverside.info,
      source_id: `source-${digest}`,
      knowledge_pages: [
        {
          page_id: "page-field-visit",
          title: "Riverside field visit",
          kind: "event",
          path: "pages/page-field-visit.md",
        },
      ],
    },
    body: `# Field report\n\n[Riverside field visit](pages/page-field-visit.md)`,
  };
  const knowledge = {
    page_id: "page-field-visit",
    source_id: source.info.source_id,
    title: "Riverside field visit",
    kind: "event",
    markdown: `---\npage_id: page-field-visit\n---\n\n# Riverside field visit\n\n## Facts\n\n- **visit count:** 12 visits\n  - Evidence: “12 visits, excluding two unverified reports.”\n  - Origin: observed · extracted projection lines 2–2, extracted projection bytes 20–68 · extracted Office projection; offsets are not original package byte offsets · original locator: DOCX table 1 row 2; channel=table\n  - The retained original opens as a fallback; the offsets above refer to the stated extracted projection.\n  - Links: [Source page](../sources/${digest}/index.md) · [Retained original](../sources/${digest}/original.docx)`,
  };
  const api = testApi();
  api.importSource = vi.fn().mockResolvedValue(source);
  api.openKnowledgePage = vi.fn().mockResolvedValue(knowledge);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();

  await user.click(screen.getByRole("button", { name: "Add source" }));
  await user.click(
    await screen.findByRole("link", { name: "Riverside field visit" }),
  );

  expect(
    await screen.findByText(
      /extracted Office projection; offsets are not original package byte offsets/,
    ),
  ).toBeTruthy();
  expect(screen.getByText(/DOCX table 1 row 2; channel=table/)).toBeTruthy();
  expect(
    screen.getByText(
      /retained original opens as a fallback.*stated extracted projection/i,
    ),
  ).toBeTruthy();
  expect(screen.getByRole("link", { name: "Retained original" })).toBeTruthy();
});
