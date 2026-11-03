import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeAll, expect, it } from "vitest";
import { screen } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { mountReader } from "../../../../src/reader";
import type { GardenApi, SourceList, SourcePage } from "../../../../src/api";

const rootDir = join(process.cwd());
const manifest = join(rootDir, "src-tauri/Cargo.toml");
const driver = join(
  process.env.CARGO_TARGET_DIR ?? join(rootDir, "src-tauri/target"),
  "debug/examples/application_driver",
);

beforeAll(() => {
  execFileSync(
    "cargo",
    [
      "build",
      "--locked",
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
});

it("local-text-import: Add source through the reader with collection side effects", async () => {
  const artifacts = process.env.KG_VERIFY_ARTIFACTS;
  if (!artifacts) {
    throw new Error("KG_VERIFY_ARTIFACTS is required (run kg-verify init-run first).");
  }
  mkdirSync(artifacts, { recursive: true });

  workspace =
    process.env.KG_VERIFY_WORKSPACE ??
    mkdtempSync(join(tmpdir(), "knowledge-garden-verify-journey-"));
  const collection =
    process.env.KG_VERIFY_COLLECTION ?? join(workspace, "collection");
  mkdirSync(collection, { recursive: true });

  const source = join(workspace, "Riverside notes.txt");
  const bytes = Buffer.from(
    "Observation V17 took place at Riverside on May 17, 2024.\r\n",
  );
  writeFileSync(source, bytes);

  const call = <T>(operation: string, ...args: string[]): T =>
    JSON.parse(
      execFileSync(driver, [collection, operation, ...args], {
        encoding: "utf8",
        timeout: 30_000,
      }),
    );

  let selected: SourcePage | undefined;
  let openedOriginal: string | undefined;
  const api: GardenApi = {
    chooseFile: async () => source,
    chooseRepositoryDirectory: async () => null,
    importSource: async (path, method) => {
      selected = call<SourcePage>("import", path, method);
      return selected;
    },
    importRepository: async () => ({
      repository_id: "repository-test",
      sources: [],
      exclusions: [],
    }),
    importUrl: async () => {
      throw new Error("URL retrieval is out of scope for local-text-import.");
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
    queryPages: async () => ({
      results: [],
      graph_revision: "revision-1",
      next_cursor: null,
      complete: true,
      work_units: 0,
      diagnostics: [],
    }),
    cancelQuery: async () => {},
    pathGraphRevision: async () => call("path-revision"),
    explorePaths: async (request) => call("paths", JSON.stringify(request)),
    explorePathDetails: async (request) =>
      call("path-details", JSON.stringify(request)),
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
    onDrop: async () => () => {},
  };

  const mountRoot = document.createElement("div");
  document.body.append(mountRoot);
  dispose = await mountReader(mountRoot, api);
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

  const pagePath = join(
    collection,
    "sources",
    selected!.info.source_id.slice("source-".length),
    "index.md",
  );
  const markdown = readFileSync(pagePath, "utf8");
  writeFileSync(join(artifacts, "source-index.md"), markdown);
  writeFileSync(
    join(artifacts, "reader-article.txt"),
    screen.getByRole("article").textContent ?? "",
  );
  writeFileSync(
    join(artifacts, "driver-list.json"),
    JSON.stringify(call<SourceList>("list", "0"), null, 2),
  );

  expect(markdown).toContain("Source text · lines 1–1");
  expect(call<SourceList>("list", "0").sources).toHaveLength(1);
  expect(readFileSync(source)).toEqual(bytes);

  dispose();
  dispose = undefined;
  rmSync(join(collection, ".derived"), { recursive: true, force: true });
  const reopened = call<SourcePage>("open", selected!.info.source_id);
  expect(reopened.body).toContain("Observation V17");
}, 120_000);
