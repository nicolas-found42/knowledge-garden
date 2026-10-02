import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeAll, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { mountReader } from "./reader";
import type { GardenApi, SourceList, SourcePage } from "./api";

const manifest = join(process.cwd(), "src-tauri/Cargo.toml");
const driver = join(
  process.cwd(),
  "src-tauri/target/debug/examples/application_driver",
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
    listSources: async (offset) => call<SourceList>("list", String(offset)),
    openSource: async (id) => call<SourcePage>("open", id),
    openOriginal: async (id) => {
      openedOriginal = call<string>("original", id);
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
