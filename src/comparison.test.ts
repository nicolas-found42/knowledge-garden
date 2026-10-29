import { afterEach, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import {
  comparisonSelection,
  renderComparison,
  type ComparisonViewState,
} from "./comparison";
import type { ComparisonSide, GardenApi } from "./api";
afterEach(() => document.body.replaceChildren());
function side(id: string, text: string, signature: string): ComparisonSide {
  return {
    source_id: id,
    title: id,
    original_name: `${id}.txt`,
    origin: `/fixtures/${id}.txt`,
    format: "txt",
    source_version_id: "v1",
    asset: "original.txt",
    signature,
    status: "fresh",
    detail: "Retained source qualifications.",
    processing_status: "complete",
    coverage: [],
    context_gaps: [],
    passage: {
      text,
      line_start: 1,
      line_end: 1,
      byte_start: 0,
      byte_end: text.length,
      offset_basis: "retained_original_text",
      next_line: null,
      truncated: false,
    },
  };
}
const state = (): ComparisonViewState => ({
  selections: [
    comparisonSelection("left", "First report"),
    comparisonSelection("right", "Second report"),
  ],
});
it("clears stale text and prevents old-original handoff while the independent side remains readable", async () => {
  let changed = false;
  const api: Pick<GardenApi, "comparisonSide" | "openComparisonOriginal"> = {
    comparisonSide: vi.fn().mockImplementation(async (request) => {
      if (request.source_id === "right")
        return side("right", "15 visits, provisional count.", "r1");
      const response = side(
        "left",
        changed ? "14 visits, corrected total." : "12 visits, two unverified.",
        changed ? "l2" : "l1",
      );
      if (changed && request.expected_signature === "l1")
        return {
          ...response,
          status: "stale",
          passage: null,
          detail: "Source changed.",
        };
      return response;
    }),
    openComparisonOriginal: vi.fn().mockImplementation(async (request) => {
      if (
        changed &&
        request.source_id === "left" &&
        request.expected_signature === "l1"
      )
        throw new Error("Source changed before original handoff");
    }),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const selected = state();
  const report = vi.fn();
  const controller = renderComparison(
    root,
    api,
    selected,
    vi.fn(),
    vi.fn(),
    report,
  );
  await controller.ready;
  const user = userEvent.setup();
  let right = screen.getByRole("region", { name: "Right source comparison" });
  expect(right.textContent).toContain("provisional count");
  const rightPassage = within(right).getByLabelText("Right source passage");
  rightPassage.scrollTop = 40;
  rightPassage.dispatchEvent(new Event("scroll"));
  changed = true;
  await user.click(screen.getByRole("button", { name: "Open left original" }));
  await screen.findByText(
    "Selection changed; its earlier passage has been cleared.",
  );
  expect(screen.queryByText("12 visits, two unverified.")).toBeNull();
  expect(
    (
      screen.getByRole("button", {
        name: "Open left original",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  right = screen.getByRole("region", { name: "Right source comparison" });
  expect(right.textContent).toContain("15 visits, provisional count.");
  expect(within(right).getByLabelText("Right source passage").scrollTop).toBe(
    40,
  );
  await user.click(
    screen.getByRole("button", { name: "Refresh left selection" }),
  );
  await screen.findByText("14 visits, corrected total.");
  await user.click(screen.getByRole("button", { name: "Open left original" }));
  expect(api.openComparisonOriginal).toHaveBeenLastCalledWith({
    source_id: "left",
    line_start: 1,
    expected_signature: "l2",
  });
});
it("revalidates restored selections before showing cached passages", async () => {
  let resolve!: (value: ComparisonSide) => void;
  const pending = new Promise<ComparisonSide>((done) => {
    resolve = done;
  });
  const selected = state();
  selected.selections[0]!.response = side(
    "left",
    "Obsolete cached wording",
    "old",
  );
  selected.selections[0]!.signature = "old";
  const api: Pick<GardenApi, "comparisonSide"> = {
    comparisonSide: vi
      .fn()
      .mockImplementation(async (request) =>
        request.source_id === "left"
          ? pending
          : side("right", "Independent report", "r1"),
      ),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const controller = renderComparison(
    root,
    api,
    selected,
    vi.fn(),
    vi.fn(),
    vi.fn(),
  );
  expect(screen.queryByText("Obsolete cached wording")).toBeNull();
  resolve({
    ...side("left", "Current wording", "new"),
    status: "stale",
    passage: null,
  });
  await controller.ready;
  expect(screen.queryByText("Obsolete cached wording")).toBeNull();
  expect(
    screen.getByText(
      "Selection changed; its earlier passage has been cleared.",
    ),
  ).toBeTruthy();
});
it("allows a later explicit retry to supersede an earlier pending response", async () => {
  let resolve!: (value: ComparisonSide) => void;
  const pending = new Promise<ComparisonSide>((done) => {
    resolve = done;
  });
  let left = 0;
  const api: Pick<GardenApi, "comparisonSide"> = {
    comparisonSide: vi
      .fn()
      .mockImplementation(async (request) =>
        request.source_id === "right"
          ? side("right", "Independent report", "r1")
          : ++left === 1
            ? pending
            : side("left", "Latest result", "l2"),
      ),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const controller = renderComparison(
    root,
    api,
    state(),
    vi.fn(),
    vi.fn(),
    vi.fn(),
  );
  const user = userEvent.setup();
  await user.click(
    screen.getByRole("button", { name: "Retry left selection" }),
  );
  await screen.findByText("Latest result");
  resolve(side("left", "Late old result", "l1"));
  await controller.ready;
  expect(screen.queryByText("Late old result")).toBeNull();
  expect(screen.getByText("Latest result")).toBeTruthy();
});
