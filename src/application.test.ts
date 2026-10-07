import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeAll, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { mountReader } from "./reader";
import type { GardenApi, KnowledgePage, SourceList, SourcePage } from "./api";

const manifest = join(process.cwd(), "src-tauri/Cargo.toml");
const driver = join(
  process.env.CARGO_TARGET_DIR ?? join(process.cwd(), "src-tauri/target"),
  "debug/examples/application_driver",
);

beforeAll(() => {
  execFileSync(
    "cargo",
    [
      "build",
      "--quiet",
      "--manifest-path",
      manifest,
      "--example",
      "application_driver",
    ],
    { timeout: 180_000 },
  );
}, 180_000);

let dispose: (() => void) | undefined;
let workspace: string | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
  localStorage.clear();
  if (workspace) rmSync(workspace, { recursive: true, force: true });
});

it("imports a real temporary source through the reader and preserves visible content, Markdown, original bytes, restart and index recovery", async () => {
  workspace = mkdtempSync(join(tmpdir(), "knowledge-garden-app-test-"));
  const collection = join(workspace, "collection");
  const source = join(workspace, "Riverside notes.txt");
  const bytes = Buffer.from(
    "Observation V17 took place at Riverside on May 17, 2024.\r\n",
  );
  writeFileSync(source, bytes);
  const call = <T>(operation: string, ...args: string[]): T =>
    JSON.parse(
      execFileSync(driver, [collection, operation, ...args], {
        encoding: "utf8",
        timeout: 10_000,
      }),
    );
  let selected: SourcePage | undefined;
  let dropFiles: ((paths: string[]) => void) | undefined;
  let openedOriginal: string | undefined;
  const api: GardenApi = {
    chooseFile: async () => source,
    importSource: async (path, method) => {
      selected = call<SourcePage>("import", path, method);
      return selected;
    },
    importUrl: async () => {
      throw new Error(
        "URL retrieval is exercised through the desktop API adapter.",
      );
    },
    listUrlAcquisitions: async () => [],
    listSources: async (offset) => call<SourceList>("list", String(offset)),
    searchPages: async () => ({
      pages: [],
      next_offset: null,
      available_tags: [],
      available_formats: [],
      available_statuses: [],
    }),
    openSource: async (id) => call<SourcePage>("open", id),
    openKnowledgePage: async (id) => call("knowledge", id),
    openOriginal: async (id) => {
      openedOriginal = call<string>("original", id);
    },
    openOriginalVersion: async (id, versionId, asset) => {
      openedOriginal = call<string>("original-version", id, versionId, asset);
    },
    openOriginalAsset: async (id, asset) => {
      openedOriginal = call<string>("original-asset", id, asset);
    },
    onDrop: async (handler) => {
      dropFiles = handler;
      return () => {};
    },
  };
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add source" }));
  await screen.findByRole("heading", { name: "Riverside notes" });
  expect(
    screen.getByText(
      "Observation V17 took place at Riverside on May 17, 2024.",
    ),
  ).toBeTruthy();
  await user.click(screen.getByRole("link", { name: "Open original" }));
  expect(readFileSync(openedOriginal!)).toEqual(bytes);
  expect(readFileSync(source)).toEqual(bytes);
  const pagePath = join(
    collection,
    "sources",
    selected!.info.source_id.slice("source-".length),
    "index.md",
  );
  const markdown = readFileSync(pagePath, "utf8");
  expect(markdown).toContain("Source text · lines 1–1");
  expect(markdown).toContain("[Open original](original.txt)");
  const identity = selected!.info.page_id;

  dispose();
  rmSync(join(collection, ".derived"), { recursive: true });
  // Each operation opens the collection in a fresh process, exercising disk restart.
  dispose = await mountReader(root, api);
  expect(screen.getByRole("heading", { name: "Riverside notes" })).toBeTruthy();
  dropFiles!([source]);
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toBe("Source added."),
  );
  expect(selected!.info.page_id).toBe(identity);
  expect(call<SourceList>("list", "0").sources).toHaveLength(1);
  expect(readFileSync(pagePath, "utf8")).toBe(markdown);
  expect(readFileSync(openedOriginal!)).toEqual(bytes);

  const unsupported = join(workspace, "unsupported.bin");
  const binary = Buffer.from([0, 255, 128, 1]);
  writeFileSync(unsupported, binary);
  dropFiles!([unsupported]);
  await screen.findByRole("heading", { name: "unsupported" });
  expect(screen.getByText(/Text is unavailable/)).toBeTruthy();
  expect(selected!.info.extraction).toBe("unsupported");
  await user.click(screen.getByRole("link", { name: "Open original" }));
  expect(readFileSync(openedOriginal!)).toEqual(binary);
}, 30_000);

it("preserves literal tool names and arguments when reading acquired conversation knowledge", async () => {
  workspace = mkdtempSync(
    join(tmpdir(), "knowledge-garden-conversation-reader-"),
  );
  const collection = join(workspace, "collection");
  const fixture = join(
    process.cwd(),
    "src-tauri/tests/fixtures/conversations/plan-rollout-shape-v2.jsonl",
  );
  const bytes = readFileSync(fixture);
  const call = <T>(operation: string, ...args: string[]): T =>
    JSON.parse(
      execFileSync(driver, [collection, operation, ...args], {
        encoding: "utf8",
        timeout: 15_000,
      }),
    );
  let imported: SourcePage | undefined;
  let original: string | undefined;
  const api: GardenApi = {
    chooseFile: async () => fixture,
    importSource: async (path) => {
      imported = call<SourcePage>("conversation-recorded-import", path);
      return imported;
    },
    importUrl: async () => {
      throw new Error("This fixture supplies no URL destination.");
    },
    listUrlAcquisitions: async () => [],
    listSources: async (offset) => call<SourceList>("list", String(offset)),
    searchPages: async () => ({
      pages: [],
      next_offset: null,
      available_tags: [],
      available_formats: [],
      available_statuses: [],
    }),
    openSource: async (id) => call<SourcePage>("open", id),
    openKnowledgePage: async (id) => call<KnowledgePage>("knowledge", id),
    openOriginal: async (id) => {
      original = call<string>("original", id);
    },
    openOriginalVersion: async (id, version, asset) => {
      original = call<string>("original-version", id, version, asset);
    },
    openOriginalAsset: async (id, asset) => {
      original = call<string>("original-asset", id, asset);
    },
    onDrop: async () => () => {},
  };
  const root = document.createElement("div");
  document.body.append(root);
  dispose = await mountReader(root, api);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add source" }));
  expect(imported?.info.semantic_state).toBe("complete");
  const label = "Message call-check-1 in 11111111-1111-4111-8111-111111111111";
  await user.click(await screen.findByRole("link", { name: label }));
  const article = await screen.findByRole("article", { name: label });
  expect(article.textContent).toContain(
    'Tool call deploy_check arguments: {"plan":"Birch"}',
  );
  expect(article.textContent).toContain(
    "not an independently observed external fact",
  );
  expect(article.textContent).toContain("tool_call");
  await user.click(
    screen.getAllByRole("link", { name: "Retained original" })[0]!,
  );
  expect(readFileSync(original!)).toEqual(bytes);
  await user.click(screen.getByRole("button", { name: "Back" }));
  expect(
    await screen.findByRole("article", { name: "plan-rollout-shape-v2" }),
  ).toBeTruthy();
  expect(readFileSync(fixture)).toEqual(bytes);
}, 30_000);

it("imports labeled DOCX and PPTX through the public reader with distinct located channels, qualifications, coverage gaps and unchanged originals", async () => {
  workspace = mkdtempSync(join(tmpdir(), "knowledge-garden-office-test-"));
  const collection = join(workspace, "collection");
  const fixtures = join(
    process.cwd(),
    "src-tauri/tests/fixtures/office/source",
  );
  const docx = join(fixtures, "field-visit.docx");
  const pptx = join(fixtures, "visit-summary.pptx");
  const call = <T>(operation: string, ...args: string[]): T =>
    JSON.parse(
      execFileSync(driver, [collection, operation, ...args], {
        encoding: "utf8",
        timeout: 10_000,
      }),
    );
  const docxBytes = readFileSync(docx);
  const pptxBytes = readFileSync(pptx);
  const doc = call<SourcePage>("import", docx, "picker");
  expect(doc.info.extraction).toBe("partial_text");
  expect(doc.body).toContain("Observed results");
  expect(doc.body).toContain("12 visits, excluding two unverified reports.");
  expect(doc.body).toContain(
    "Counts include only visits confirmed by two observers.",
  );
  expect(doc.body).toContain("Measure");
  expect(doc.body).toContain("Result");
  expect(doc.body).toContain("https://example.invalid/unacquired-methods");
  expect(doc.body).not.toContain("unacquired methods were fetched");
  expect(doc.body).toContain("embedded object");
  expect(doc.info.extraction_coverage).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ scope: "main_document", status: "partial" }),
      expect.objectContaining({ scope: "tables", status: "complete" }),
      expect.objectContaining({ scope: "embedded_object", status: "partial" }),
    ]),
  );
  expect(readFileSync(call<string>("original", doc.info.source_id))).toEqual(
    docxBytes,
  );

  const deck = call<SourcePage>("import", pptx, "picker");
  expect(deck.info.extraction).toBe("partial_text");
  expect(deck.body).toContain("Slide 1");
  expect(deck.body).toContain("Visible slide text");
  expect(deck.body).toContain("12 visits");
  expect(deck.body).toContain("Speaker notes");
  expect(deck.body).toContain(
    "An unconfirmed correction suggests the count may be 14 visits.",
  );
  expect(deck.body.indexOf("Visible slide text")).toBeLessThan(
    deck.body.indexOf("Speaker notes"),
  );
  expect(deck.body).toContain("https://example.invalid/linked-method");
  expect(deck.info.extraction_coverage).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ scope: "slide_text", status: "complete" }),
      expect.objectContaining({ scope: "speaker_notes", status: "complete" }),
      expect.objectContaining({ scope: "embedded_object", status: "partial" }),
    ]),
  );
  expect(readFileSync(call<string>("original", deck.info.source_id))).toEqual(
    pptxBytes,
  );

  const reopened = call<SourcePage>("open", deck.info.source_id);
  expect(reopened.body).toBe(deck.body);
  for (const [fixture, imported] of [
    [docx, doc],
    [pptx, deck],
  ] as const) {
    const semantic = call<SourcePage>(
      "office-recorded-import",
      fixture,
      "picker",
    );
    expect(semantic.info.semantic_state).toBe("complete");
    expect(semantic.info.knowledge_pages).toHaveLength(1);
    const evidencePage = call<{
      markdown: string;
    }>("knowledge", semantic.info.knowledge_pages[0].page_id);
    expect(evidencePage.markdown).toContain(
      "offset_basis: extracted_office_projection",
    );
    expect(evidencePage.markdown).toContain("source_location:");
    expect(evidencePage.markdown).toContain(
      fixture.endsWith(".docx")
        ? "DOCX table 1 row 2; channel=table"
        : "PPTX slide 1 speaker note 1",
    );
    expect(evidencePage.markdown).toContain(
      "offsets are not original package byte offsets",
    );
    expect(evidencePage.markdown).toContain("extracted projection lines");
    expect(evidencePage.markdown).toContain(
      "The retained original opens as a fallback",
    );
    if (fixture.endsWith(".pptx")) {
      expect(evidencePage.markdown).toContain(
        "unconfirmed note, not visible slide text",
      );
    } else {
      expect(evidencePage.markdown).toContain(
        "12 visits, excluding two unverified reports",
      );
    }
    expect(evidencePage.markdown).toContain("[Source page](../sources/");
    expect(evidencePage.markdown).toContain("[Retained original](../sources/");
    expect(semantic.info.source_id).toBe(imported.info.source_id);
  }
  expect(readFileSync(docx)).toEqual(docxBytes);
  expect(readFileSync(pptx)).toEqual(pptxBytes);
}, 30_000);

it("keeps damaged Office originals inspectable and recovers an interrupted Office semantic claim without losing extraction coverage", async () => {
  workspace = mkdtempSync(join(tmpdir(), "knowledge-garden-office-recovery-"));
  const fixtures = join(
    process.cwd(),
    "src-tauri/tests/fixtures/office/source",
  );
  const source = join(fixtures, "field-visit.docx");
  const originalBytes = readFileSync(source);
  const damaged = join(workspace, "damaged.docx");
  writeFileSync(damaged, originalBytes.subarray(0, 64));
  const call = <T>(
    collection: string,
    operation: string,
    ...args: string[]
  ): T =>
    JSON.parse(
      execFileSync(driver, [collection, operation, ...args], {
        encoding: "utf8",
        timeout: 10_000,
      }),
    );
  const collection = join(workspace, "collection");
  const broken = call<SourcePage>(collection, "import", damaged, "picker");
  expect(broken.info.extraction).toBe("invalid_container");
  expect(broken.info.extraction_coverage).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ scope: "main_document", status: "failed" }),
    ]),
  );
  expect(broken.body).toContain("Text is unavailable");
  expect(
    readFileSync(call<string>(collection, "original", broken.info.source_id)),
  ).toEqual(originalBytes.subarray(0, 64));

  const interruptedCollection = join(workspace, "interrupted-collection");
  const office = call<SourcePage>(
    interruptedCollection,
    "import",
    source,
    "picker",
  );
  const claimed = call<{ source_id: string; source_text: string }[]>(
    interruptedCollection,
    "claim",
  );
  expect(claimed).toHaveLength(1);
  expect(claimed[0].source_id).toBe(office.info.source_id);
  expect(claimed[0].source_text).toContain(
    "[DOCX table 1 row 2; channel=table]",
  );
  const recovered = call<SourcePage>(
    interruptedCollection,
    "recover-open",
    office.info.source_id,
  );
  expect(recovered.info.semantic_state).toBe("pending");
  expect(recovered.info.semantic_error).toContain("interrupted");
  expect(recovered.info.extraction_coverage).toEqual(
    office.info.extraction_coverage,
  );
  expect(recovered.body).toContain(
    "12 visits, excluding two unverified reports.",
  );
  expect(
    readFileSync(
      call<string>(interruptedCollection, "original", office.info.source_id),
    ),
  ).toEqual(originalBytes);
}, 30_000);
