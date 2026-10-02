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
};

function testApi(): GardenApi {
  return {
    chooseFile: vi.fn().mockResolvedValue("/tmp/Riverside notes.md"),
    importSource: vi.fn().mockResolvedValue(riverside),
    listSources: vi.fn().mockResolvedValue({ sources: [], next_offset: null }),
    openSource: vi.fn().mockResolvedValue(riverside),
    openOriginal: vi.fn().mockResolvedValue(undefined),
    onDrop: vi.fn().mockResolvedValue(() => {}),
  };
}

let dispose: (() => void) | undefined;
afterEach(() => {
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
