import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { mountReader } from "./reader";
import type { GardenApi, SourcePage } from "./api";

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
    listSources: vi.fn().mockResolvedValue({ sources: [], next_offset: null }),
    openSource: vi.fn().mockResolvedValue(riverside),
    openKnowledgePage: vi.fn(),
    openOriginal: vi.fn().mockResolvedValue(undefined),
    onDrop: vi.fn().mockResolvedValue(() => {}),
  };
}

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
  await user.click(screen.getByRole("link", { name: "Open original" }));
  expect(api.openOriginal).toHaveBeenCalledWith(source.info.source_id);
});
