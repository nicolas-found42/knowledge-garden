import { afterEach, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/dom";
import userEvent from "@testing-library/user-event";
import { trailPanel } from "./trails";
import type { GardenApi, TrailStop, ResolvedTrailStop } from "./api";
const source = "source-" + "a".repeat(64),
  version = "b".repeat(64);
function stop(label: string, context: TrailStop["context"] = null): TrailStop {
  return {
    stop_id: crypto.randomUUID(),
    label,
    target: { source_id: source, page_id: null, title: label },
    context,
  };
}
const resolved = (s: TrailStop): ResolvedTrailStop => ({
  stop: s,
  status: "available",
  title: s.label,
  detail: "Current material",
  quote: null,
  locator: null,
  retained_asset: null,
});
afterEach(() => document.body.replaceChildren());
function setup() {
  const host = document.createElement("aside");
  document.body.append(host);
  const state = {
    current: stop("Riverside"),
    intent: 0,
    quote: "Unique wording from the source.",
  };
  const api = {
    resolveTrail: vi
      .fn()
      .mockImplementation(async (stops: TrailStop[]) => stops.map(resolved)),
    trailSourceChoices: vi.fn().mockResolvedValue({
      stops: [
        stop("Message choice", {
          kind: "message",
          source_version_id: version,
          message_id: "choice",
        }),
        stop("Recording 500 ms", {
          kind: "media",
          source_version_id: version,
          at_ms: 500,
          channel: "audio",
        }),
      ],
      next_offset: null,
      detail: "Acquired locations",
    }),
    trailSelectedPassage: vi.fn().mockResolvedValue(
      stop("Source passage", {
        kind: "span",
        source_version_id: version,
        basis: "retained_original_text",
        byte_start: 0,
        byte_end: 30,
        quote: "Unique wording from the source.",
      }),
    ),
    suggestTrailStops: vi.fn().mockResolvedValue({
      stops: [stop("Related observation")],
      judgments: [0.93],
      model: "typesafe/jev-1.14-recorded",
      detail: "Existing material only.",
    }),
  } as Pick<
    GardenApi,
    | "resolveTrail"
    | "trailSourceChoices"
    | "trailSelectedPassage"
    | "suggestTrailStops"
  >;
  const callbacks = {
    current: () => state.current,
    source: () => source,
    selection: () => state.quote,
    begin: () => ++state.intent,
    cancel: () => {
      state.intent++;
    },
    active: (intent: number) => intent === state.intent,
    follow: vi.fn().mockResolvedValue(true),
    origin: vi.fn(),
    returnOrigin: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined),
    error: vi.fn(),
  };
  const panel = trailPanel(host, api as GardenApi, callbacks);
  panel.show();
  return { panel, api, callbacks, state };
}
it("assembles mixed stops, keeps duplicate visits distinct, reorders by keyboard, traverses and explicitly saves the route", async () => {
  const { panel, api, callbacks, state } = setup();
  const user = userEvent.setup();
  await user.click(
    screen.getByRole("button", { name: "Add current page to trail" }),
  );
  state.current = stop("V17 event");
  await user.click(
    screen.getByRole("button", { name: "Add current page to trail" }),
  );
  const move = screen.getByRole("button", { name: "Move down stop 1" });
  move.focus();
  await user.keyboard("{Enter}");
  expect(document.activeElement?.getAttribute("aria-label")).toBe(
    "Read trail stop 2: Riverside",
  );
  expect(panel.stops().map((s) => s.label)).toEqual(["V17 event", "Riverside"]);
  await user.click(
    screen.getByRole("button", {
      name: "Choose source message or media location",
    }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Add Message choice" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Add Recording 500 ms" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Add selected source passage" }),
  );
  await waitFor(() => expect(panel.stops()).toHaveLength(5));
  expect(api.trailSelectedPassage).toHaveBeenCalledWith(source, state.quote);
  await user.click(
    screen.getByRole("button", { name: "Read trail stop 1: V17 event" }),
  );
  await waitFor(() => expect(callbacks.follow).toHaveBeenCalledOnce());
  await user.click(screen.getByRole("button", { name: "Next trail stop" }));
  await waitFor(() => expect(callbacks.follow).toHaveBeenCalledTimes(2));
  expect(callbacks.follow.mock.calls[1][0].stop.label).toBe("Riverside");
  await user.click(screen.getByRole("button", { name: "Previous trail stop" }));
  await waitFor(() => expect(callbacks.follow).toHaveBeenCalledTimes(3));
  await user.click(
    screen.getByRole("button", { name: "Return to trail origin" }),
  );
  await waitFor(() => expect(callbacks.returnOrigin).toHaveBeenCalledOnce());
  expect(document.activeElement?.textContent).toBe("Return to trail origin");
  await user.click(screen.getByRole("button", { name: "Remove trail stop 2" }));
  expect(panel.stops().map((s) => s.label)).toEqual([
    "V17 event",
    "Message choice",
    "Recording 500 ms",
    "Source passage",
  ]);
  expect(callbacks.save).not.toHaveBeenCalled();
  await user.type(screen.getByLabelText("Trail name"), "River study");
  await user.click(screen.getByRole("button", { name: "Save this trail" }));
  await waitFor(() =>
    expect(callbacks.save).toHaveBeenCalledWith("River study", panel.stops()),
  );
  // Revisiting one location is a distinct ordered stop, never a duplicate stable stop ID.
  await user.click(
    screen.getByRole("button", {
      name: "Choose source message or media location",
    }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Add Message choice" }),
  );
  const ids = panel.stops().map((s) => s.stop_id);
  expect(new Set(ids).size).toBe(ids.length);
});
it("cancels a pending stop resolution when the route is edited and discloses unavailable reopened stops", async () => {
  const { panel, api, callbacks } = setup();
  const user = userEvent.setup();
  await user.click(
    screen.getByRole("button", { name: "Add current page to trail" }),
  );
  let finish!: (value: ResolvedTrailStop[]) => void;
  vi.mocked(api.resolveTrail!).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const selected = panel.stops()[0];
  await user.click(
    screen.getByRole("button", { name: "Read trail stop 1: Riverside" }),
  );
  await user.click(screen.getByRole("button", { name: "Remove trail stop 1" }));
  finish([resolved(selected)]);
  await Promise.resolve();
  expect(callbacks.follow).not.toHaveBeenCalled();
  vi.mocked(api.resolveTrail!).mockResolvedValue([
    {
      ...resolved(selected),
      status: "unavailable",
      detail: "Source was removed; no replacement selected.",
    },
  ]);
  panel.replace([selected]);
  await screen.findByText(/unavailable: Riverside.*Source was removed/);
  expect(panel.stops()[0].stop_id).toBe(selected.stop_id);
});
it("offers only existing Jev-selected stops and adds none until the owner chooses one", async () => {
  const { panel, callbacks } = setup();
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("Reading goal"), "Riverside");
  await user.click(
    screen.getByRole("button", { name: "Suggest existing stops" }),
  );
  const add = await screen.findByRole("button", {
    name: "Add suggested Related observation",
  });
  expect(panel.stops()).toEqual([]);
  expect(callbacks.save).not.toHaveBeenCalled();
  await user.click(add);
  expect(panel.stops()[0].label).toBe("Related observation");
  expect(screen.getByText(/typesafe\/jev-1.14-recorded/)).toBeTruthy();
});

it("clears changed-stop fallback actions when a different available stop is followed", async () => {
  const { panel, api, callbacks } = setup();
  const user = userEvent.setup();
  const changed = stop("Changed A", {
    kind: "media",
    source_version_id: version,
    at_ms: 500,
    channel: "audio",
  });
  const available = stop("Available B");
  const current = (s: TrailStop): ResolvedTrailStop =>
    s.stop_id === changed.stop_id
      ? {
          ...resolved(s),
          status: "changed",
          detail: "A has a newer version.",
          retained_asset: "original.wav",
        }
      : resolved(s);
  vi.mocked(api.resolveTrail!).mockImplementation(async (stops) =>
    stops.map(current),
  );
  panel.replace([changed, available]);
  await user.click(
    screen.getByRole("button", { name: "Read trail stop 1: Changed A" }),
  );
  await screen.findByRole("button", { name: "Open current source separately" });
  expect(
    screen.getByRole("button", { name: "Open exact saved original" }),
  ).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Read trail stop 2: Available B" }),
  );
  await waitFor(() => expect(callbacks.follow).toHaveBeenCalled());
  expect(
    screen.queryByRole("button", { name: "Open current source separately" }),
  ).toBeNull();
  expect(
    screen.queryByRole("button", { name: "Open exact saved original" }),
  ).toBeNull();
});

it("refreshes a saved stop row when its current availability changes during traversal", async () => {
  const { panel, api } = setup();
  const user = userEvent.setup();
  const selected = stop("Formerly available");
  panel.replace([selected]);
  await screen.findByText(/^available: Formerly available/);
  vi.mocked(api.resolveTrail!).mockResolvedValue([
    {
      ...resolved(selected),
      status: "unavailable",
      detail: "This source was removed.",
    },
  ]);
  await user.click(
    screen.getByRole("button", {
      name: "Read trail stop 1: Formerly available",
    }),
  );
  await screen.findByText(/^unavailable: Formerly available/);
  expect(screen.queryByText(/^available: Formerly available/)).toBeNull();
});
