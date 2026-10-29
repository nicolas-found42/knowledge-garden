import { afterEach, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { gapInspector } from "./gaps";
import type { GapInspection, KnowledgeGap } from "./api";
afterEach(() => document.body.replaceChildren());
const gap = (category: KnowledgeGap["category"]): KnowledgeGap => ({
  category,
  scope: "V17 pages 3–4",
  detail: "Missing extraction; earlier supported content stays current.",
  recovery: "Automatic retry scheduled.",
  source_id: "source-test",
  original_available: true,
  original_version: null,
  evidence: "12 visits, excluding two unverified reports.",
});
it("inspects on demand and narrows recovered gaps without hiding independent unresolved support", async () => {
  let recovered = false;
  const api = {
    inspectGaps: vi.fn(async () => ({
      gaps: recovered
        ? [gap("insufficient_support")]
        : [gap("extraction_incomplete"), gap("insufficient_support")],
      truncated: false,
    })),
    openOriginal: vi.fn(async () => {}),
    openOriginalVersion: vi.fn(async () => {}),
  };
  const panel = gapInspector(api, "source-test", vi.fn());
  document.body.append(panel);
  expect(api.inspectGaps).not.toHaveBeenCalled();
  const user = userEvent.setup();
  await user.click(screen.getByText("Inspect knowledge gaps"));
  await screen.findByRole("heading", { name: "Incomplete extraction" });
  expect(
    screen.getByRole("heading", { name: "Insufficient or qualified support" }),
  ).toBeTruthy();
  const section = screen.getByRole("heading", {
    name: "Incomplete extraction",
  }).parentElement!;
  await user.click(
    within(section).getByRole("button", { name: "Open gap source original" }),
  );
  expect(api.openOriginal).toHaveBeenCalledWith("source-test");
  recovered = true;
  await user.click(
    screen.getByRole("button", { name: "Refresh gap inspection" }),
  );
  await screen.findByText("1 gap explanations.");
  expect(
    screen.queryByRole("heading", { name: "Incomplete extraction" }),
  ).toBeNull();
  expect(
    screen.getByRole("heading", { name: "Insufficient or qualified support" }),
  ).toBeTruthy();
});
it("opens a relationship's precise retained version instead of the unrelated current original", async () => {
  const item = gap("insufficient_support");
  item.original_version = ["revision-1", "original.txt"];
  const api = {
    inspectGaps: async () => ({ gaps: [item], truncated: false }),
    openOriginal: vi.fn(async () => {}),
    openOriginalVersion: vi.fn(async () => {}),
  };
  document.body.append(gapInspector(api, "page-test", vi.fn()));
  const user = userEvent.setup();
  await user.click(screen.getByText("Inspect knowledge gaps"));
  await screen.findByText("1 gap explanations.");
  await user.click(
    screen.getByRole("button", { name: "Open gap source original" }),
  );
  expect(api.openOriginalVersion).toHaveBeenCalledWith(
    "source-test",
    "revision-1",
    "original.txt",
  );
  expect(api.openOriginal).not.toHaveBeenCalled();
});
it("rejects an older gap response and keeps a failed inspection distinct from no gaps", async () => {
  let resolve!: (r: GapInspection) => void;
  const pending = new Promise<GapInspection>((r) => (resolve = r));
  let calls = 0;
  const api = {
    inspectGaps: vi.fn(async () => {
      if (++calls === 1) return pending;
      throw new Error("Missing source metadata");
    }),
    openOriginal: vi.fn(async () => {}),
    openOriginalVersion: vi.fn(async () => {}),
  };
  document.body.append(gapInspector(api, "page-test", vi.fn()));
  const user = userEvent.setup();
  await user.click(screen.getByText("Inspect knowledge gaps"));
  await user.click(
    screen.getByRole("button", { name: "Refresh gap inspection" }),
  );
  await screen.findByText(/Gap inspection unavailable/);
  resolve({ gaps: [], truncated: false });
  await Promise.resolve();
  expect(
    screen.queryByText("No gaps reported for this page's inspected scope."),
  ).toBeNull();
});
