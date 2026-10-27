import { afterEach, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { newCollectionView, renderCollection } from "./collection";
import type { CollectionItem, GardenApi } from "./api";
afterEach(() => document.body.replaceChildren());
const item = (id: number): CollectionItem => ({
  page_id: `page-${id}`,
  source_id: `source-${id}`,
  page_type: "source",
  title: `Report ${id}`,
  kind: "source",
  format: "txt",
  processing_status: "complete",
});
const listing = {
  groups: [
    { group_id: "tag:river", label: "River", members: 55 },
    { group_id: "tag:research", label: "Research", members: 55 },
  ],
  next_offset: null,
  revision: "r1",
};
it("continues bounded member windows and retains selection, expansion and draft filter across presentations", async () => {
  const api: Pick<GardenApi, "collectionGroups" | "collectionMembers"> = {
    collectionGroups: vi.fn().mockResolvedValue(listing),
    collectionMembers: vi.fn().mockImplementation(async (request) => ({
      items:
        request.offset === 0
          ? Array.from({ length: 50 }, (_, i) => item(i))
          : [item(50), item(51), item(52), item(53), item(54)],
      next_offset: request.offset === 0 ? 50 : null,
      revision: "r1",
      changed: false,
    })),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const state = newCollectionView();
  const read = vi.fn();
  const report = vi.fn();
  renderCollection(root, api, state, read, report);
  const user = userEvent.setup();
  const group = await screen.findByRole("button", { name: "River · 55 items" });
  await user.click(group);
  await screen.findByRole("button", { name: "Report 0" });
  await user.click(
    screen.getByRole("button", { name: "More members of River" }),
  );
  await screen.findByRole("button", { name: "Report 54" });
  expect(screen.queryByRole("button", { name: "Report 0" })).toBeNull();
  await user.click(screen.getByRole("button", { name: "Report 54" }));
  expect(read).toHaveBeenCalledWith(item(54));
  const filter = screen.getByRole("searchbox");
  await user.type(filter, "unsubmitted filter");
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Collection presentation" }),
    "canvas",
  );
  expect((screen.getByRole("searchbox") as HTMLInputElement).value).toBe(
    "unsubmitted filter",
  );
  expect(
    screen.getByRole("button", { name: "Read selected: Report 54" }),
  ).toBeTruthy();
  expect(root.querySelectorAll("svg")).toHaveLength(2);
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Collection presentation" }),
    "research",
  );
  expect(
    screen
      .getByRole("button", { name: "Report 54" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  expect(state.expanded["tag:river"].offset).toBe(50);
  expect(report).not.toHaveBeenCalled();
});
it("does not let a late group-member response cross into a changed axis", async () => {
  let resolve!: (value: unknown) => void;
  const pending = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  const api: Pick<GardenApi, "collectionGroups" | "collectionMembers"> = {
    collectionGroups: vi.fn().mockImplementation(async (request) =>
      request.axis === "topic"
        ? listing
        : {
            groups: [{ group_id: "subject:maya", label: "Maya", members: 1 }],
            revision: "r2",
            next_offset: null,
          },
    ),
    collectionMembers: vi.fn().mockReturnValue(pending),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const state = newCollectionView();
  renderCollection(root, api, state, vi.fn(), vi.fn());
  const user = userEvent.setup();
  await user.click(
    await screen.findByRole("button", { name: "River · 55 items" }),
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Group collection by" }),
    "person",
  );
  await screen.findByRole("button", { name: "Maya · 1 items" });
  resolve({
    items: [item(0)],
    revision: "r1",
    next_offset: null,
    changed: false,
  });
  await pending;
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "Report 0" })).toBeNull(),
  );
  expect(state.expanded["tag:river"]).toBeUndefined();
});

it("keeps a pending expansion through an unchanged automatic refresh", async () => {
  let resolve!: (value: unknown) => void;
  const pending = new Promise((done) => {
    resolve = done;
  });
  const api: Pick<GardenApi, "collectionGroups" | "collectionMembers"> = {
    collectionGroups: vi.fn().mockResolvedValue(listing),
    collectionMembers: vi.fn().mockReturnValue(pending),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const state = newCollectionView();
  const controller = renderCollection(root, api, state, vi.fn(), vi.fn());
  const user = userEvent.setup();
  await user.click(
    await screen.findByRole("button", { name: "River · 55 items" }),
  );
  await controller.refresh();
  resolve({
    items: [item(0)],
    next_offset: null,
    revision: "r1",
    changed: false,
  });
  await pending;
  expect(await screen.findByRole("button", { name: "Report 0" })).toBeTruthy();
});
it("keeps a group collapsed when its earlier member continuation finishes", async () => {
  let resolve!: (value: unknown) => void;
  const pending = new Promise((done) => {
    resolve = done;
  });
  const api: Pick<GardenApi, "collectionGroups" | "collectionMembers"> = {
    collectionGroups: vi.fn().mockResolvedValue(listing),
    collectionMembers: vi
      .fn()
      .mockResolvedValueOnce({
        items: [item(0)],
        next_offset: 50,
        revision: "r1",
        changed: false,
      })
      .mockReturnValueOnce(pending),
  };
  const root = document.createElement("main");
  document.body.append(root);
  const state = newCollectionView();
  renderCollection(root, api, state, vi.fn(), vi.fn());
  const user = userEvent.setup();
  await user.click(
    await screen.findByRole("button", { name: "River · 55 items" }),
  );
  await user.click(
    await screen.findByRole("button", { name: "More members of River" }),
  );
  await user.click(screen.getByRole("button", { name: "River · 55 items" }));
  resolve({
    items: [item(50)],
    next_offset: null,
    revision: "r1",
    changed: false,
  });
  await pending;
  expect(
    screen
      .getByRole("button", { name: "River · 55 items" })
      .getAttribute("aria-expanded"),
  ).toBe("false");
  expect(screen.queryByRole("button", { name: "Report 50" })).toBeNull();
});
