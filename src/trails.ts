import type { GardenApi, TrailStop, ResolvedTrailStop } from "./api";
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) => {
  const n = document.createElement(tag);
  if (text !== undefined) n.textContent = text;
  return n;
};
type Callbacks = {
  current: () => TrailStop | null;
  selection: () => string;
  source: () => string | null;
  begin: () => number;
  cancel: () => void;
  active: (intent: number) => boolean;
  follow: (stop: ResolvedTrailStop, intent: number) => Promise<boolean>;
  origin: () => void;
  returnOrigin: () => Promise<void>;
  save: (name: string, stops: TrailStop[]) => Promise<void>;
  error: (error: unknown) => void;
};
/** The route and its position stay temporary until the owner explicitly saves. */
export function trailPanel(
  host: HTMLElement,
  api: GardenApi,
  callbacks: Callbacks,
) {
  host.hidden = true;
  host.className = "trail-panel";
  host.setAttribute("aria-label", "Trail");
  const resolvedStops = new Map<string, ResolvedTrailStop>();
  let stops: TrailStop[] = [],
    position = -1,
    generation = 0,
    hasOrigin = false;
  function invalidate() {
    callbacks.cancel();
    return ++generation;
  }
  const heading = node("h2", "Trail");
  heading.tabIndex = -1;
  const status = node("p");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const list = node("ol"),
    choices = node("section"),
    suggestions = node("section");
  const name = node("input");
  name.required = true;
  name.maxLength = 120;
  const nameLabel = node("label", "Trail name ");
  nameLabel.append(name);
  const save = node("form");
  const submit = node("button", "Save this trail");
  submit.type = "submit";
  save.append(nameLabel, submit);
  const goal = node("input");
  goal.required = true;
  goal.maxLength = 1024;
  const goalLabel = node("label", "Reading goal ");
  goalLabel.append(goal);
  const suggest = node("form");
  const suggestButton = node("button", "Suggest existing stops");
  suggestButton.type = "submit";
  suggest.append(goalLabel, suggestButton);
  function button(text: string, action: () => void, id?: string) {
    const b = node("button", text);
    b.type = "button";
    if (id) b.dataset.focusId = id;
    b.addEventListener("click", () => {
      b.focus();
      action();
    });
    return b;
  }
  function error(reason: unknown) {
    status.textContent = String(reason);
    callbacks.error(reason);
  }
  const back = button("Previous trail stop", () => {
    void follow(position - 1);
  });
  const forward = button("Next trail stop", () => {
    void follow(position + 1);
  });
  const returnButton = button("Return to trail origin", () => {
    invalidate();
    void callbacks
      .returnOrigin()
      .then(() => {
        returnButton.focus();
        status.textContent = "Returned to the originating view.";
      })
      .catch(error);
  });
  const add = button(
    "Add current page to trail",
    () => {
      const stop = callbacks.current();
      if (stop) addStop(stop);
      else status.textContent = "Open an existing page or source to add it.";
    },
    "trail-add-current",
  );
  let selectedWording = "";
  const passage = button("Add selected source passage", () => {
    const source = callbacks.source();
    const quote = selectedWording || callbacks.selection();
    selectedWording = "";
    if (!source || !quote) {
      status.textContent =
        "Select unique wording in an open source, then add the selected passage.";
      return;
    }
    const token = invalidate();
    void api.trailSelectedPassage!(source, quote)
      .then((stop) => {
        if (token === generation) addStop(stop);
      })
      .catch((reason) => {
        if (token === generation) error(reason);
      });
  });
  passage.addEventListener("pointerdown", () => {
    selectedWording = callbacks.selection();
  });
  const choose = button("Choose source message or media location", () => {
    const id = callbacks.source();
    if (!id) {
      status.textContent =
        "Open a source to choose one of its messages or decoded media locations.";
      return;
    }
    void loadChoices(id, 0);
  });
  host.replaceChildren(
    heading,
    button("Close trail", () => {
      invalidate();
      host.hidden = true;
    }),
    status,
    add,
    passage,
    choose,
    back,
    forward,
    returnButton,
    save,
    suggest,
    suggestions,
    list,
    choices,
  );
  function addStop(stop: TrailStop) {
    if (stops.length >= 50) {
      status.textContent = "A trail supports at most 50 stops.";
      return;
    }
    invalidate();
    stops.push({ ...structuredClone(stop), stop_id: crypto.randomUUID() });
    status.textContent = `Added ${stop.label}. The trail is temporary until saved.`;
    draw();
  }
  function draw(focusId?: string) {
    list.replaceChildren();
    stops.forEach((stop, index) => {
      const row = node("li");
      row.append(node("span", stop.label));
      const resolved = resolvedStops.get(stop.stop_id);
      if (resolved)
        row.append(
          node(
            "p",
            `${resolved.status}: ${resolved.title}. ${resolved.detail}`,
          ),
        );
      const open = button(
        "Read",
        () => {
          void follow(index);
        },
        `trail-stop-${stop.stop_id}`,
      );
      open.setAttribute(
        "aria-label",
        `Read trail stop ${index + 1}: ${stop.label}`,
      );
      if (index === position) open.setAttribute("aria-current", "step");
      row.append(open);
      for (const [label, delta] of [
        ["Move up", -1],
        ["Move down", 1],
      ] as const) {
        const move = button(
          delta < 0 ? "Up" : "Down",
          () => {
            const target = index + delta;
            invalidate();
            [stops[index], stops[target]] = [stops[target], stops[index]];
            if (position === index) position = target;
            else if (position === target) position = index;
            draw(`trail-stop-${stop.stop_id}`);
            status.textContent = `Moved ${stop.label} to position ${target + 1} of ${stops.length}.`;
          },
          `trail-${label}-${stop.stop_id}`,
        );
        move.setAttribute("aria-label", `${label} stop ${index + 1}`);
        move.disabled = index + delta < 0 || index + delta >= stops.length;
        row.append(move);
      }
      const remove = button("Remove", () => {
        invalidate();
        stops.splice(index, 1);
        if (position === index) position = -1;
        else if (position > index) position--;
        draw(
          stops[Math.min(index, stops.length - 1)]
            ? `trail-stop-${stops[Math.min(index, stops.length - 1)].stop_id}`
            : "trail-add-current",
        );
        status.textContent = `Removed ${stop.label}. ${stops.length} stops remain.`;
      });
      remove.setAttribute("aria-label", `Remove trail stop ${index + 1}`);
      row.append(remove);
      list.append(row);
    });
    back.disabled = position <= 0;
    forward.disabled = stops.length === 0 || position >= stops.length - 1;
    returnButton.disabled = !hasOrigin;
    submit.disabled = stops.length === 0;
    if (focusId)
      host
        .querySelector<HTMLElement>(`[data-focus-id="${CSS.escape(focusId)}"]`)
        ?.focus();
  }
  async function follow(index: number) {
    if (index < 0 || index >= stops.length) return;
    const token = invalidate();
    choices.replaceChildren();
    const intent = callbacks.begin();
    if (!hasOrigin) {
      callbacks.origin();
      hasOrigin = true;
    }
    const selected = stops[index];
    position = index;
    draw();
    status.textContent = `Resolving selected route stop ${index + 1} of ${stops.length}…`;
    try {
      const [resolved] = await api.resolveTrail!([selected]);
      if (token !== generation || !callbacks.active(intent)) return;
      resolvedStops.set(selected.stop_id, resolved);
      draw(`trail-stop-${selected.stop_id}`);
      if (resolved.status !== "available") {
        status.textContent = `Stop ${index + 1} of ${stops.length}: ${resolved.detail}`;
        if (resolved.stop.target.source_id && resolved.status === "changed") {
          choices.append(
            button("Open current source separately", () => {
              const target = {
                ...resolved,
                stop: { ...resolved.stop, context: null },
                status: "available",
                quote: null,
                locator: null,
              };
              const next = callbacks.begin();
              void callbacks.follow(target, next).catch(error);
            }),
          );
          if (resolved.retained_asset && resolved.stop.context)
            choices.append(
              button("Open exact saved original", () => {
                void api
                  .openOriginalVersion(
                    resolved.stop.target.source_id!,
                    resolved.stop.context!.source_version_id,
                    resolved.retained_asset!,
                  )
                  .catch(error);
              }),
            );
        }
        return;
      }
      const applied = await callbacks.follow(resolved, intent);
      if (token !== generation || (!callbacks.active(intent) && !applied))
        return;
      if (applied) {
        status.textContent = `Selected route stop ${index + 1} of ${stops.length}: ${resolved.title}. ${resolved.locator ?? resolved.detail}`;
        host
          .querySelector<HTMLElement>(
            `[data-focus-id="trail-stop-${CSS.escape(selected.stop_id)}"]`,
          )
          ?.focus();
      }
    } catch (reason) {
      if (token === generation && callbacks.active(intent)) error(reason);
    }
  }
  async function loadChoices(sourceId: string, offset: number) {
    const token = invalidate();
    try {
      const result = await api.trailSourceChoices!(sourceId, offset);
      if (token !== generation || callbacks.source() !== sourceId) return;
      choices.replaceChildren(
        node("h3", "Existing source locations"),
        node("p", result.detail),
      );
      for (const stop of result.stops)
        choices.append(button(`Add ${stop.label}`, () => addStop(stop)));
      if (offset > 0)
        choices.append(
          button("Previous source locations", () => {
            void loadChoices(sourceId, Math.max(0, offset - 50));
          }),
        );
      if (result.next_offset !== null)
        choices.append(
          button("More source locations", () => {
            void loadChoices(sourceId, result.next_offset!);
          }),
        );
    } catch (reason) {
      if (token === generation) error(reason);
    }
  }
  save.addEventListener("submit", (event) => {
    event.preventDefault();
    if (!stops.length) return;
    submit.disabled = true;
    const snapshot = structuredClone(stops);
    const submittedName = name.value;
    void callbacks
      .save(submittedName, snapshot)
      .then(() => {
        status.textContent = `Saved trail “${submittedName}”. Find and manage it in Saved views.`;
        if (name.value === submittedName) name.value = "";
      })
      .catch(error)
      .finally(() => {
        submit.disabled = stops.length === 0;
      });
  });
  suggest.addEventListener("submit", (event) => {
    event.preventDefault();
    const token = invalidate();
    suggestButton.disabled = true;
    status.textContent = "Jev is assessing existing candidate stops…";
    void api.suggestTrailStops!(goal.value)
      .then((result) => {
        if (token !== generation) return;
        suggestions.replaceChildren(node("p", result.detail));
        if (result.model)
          suggestions.append(node("p", `Selection model: ${result.model}`));
        result.stops.forEach((stop, index) =>
          suggestions.append(
            button(`Add suggested ${stop.label}`, () => addStop(stop)),
            node(
              "span",
              ` Estimated relevance ${result.judgments[index].toFixed(2)}. `,
            ),
          ),
        );
        if (!result.stops.length)
          status.textContent = "No stops were added. " + result.detail;
        else
          status.textContent = `${result.stops.length} existing suggested stops. Choose which to add.`;
      })
      .catch((reason) => {
        if (token === generation) error(reason);
      })
      .finally(() => {
        suggestButton.disabled = false;
      });
  });
  draw();
  return {
    show() {
      host.hidden = false;
      heading.focus();
    },
    toggle() {
      host.hidden = !host.hidden;
      if (!host.hidden) heading.focus();
    },
    replace(next: TrailStop[]) {
      invalidate();
      stops = structuredClone(next);
      resolvedStops.clear();
      const token = generation;
      void api.resolveTrail!(structuredClone(stops))
        .then((resolved) => {
          if (token !== generation) return;
          for (const stop of resolved)
            resolvedStops.set(stop.stop.stop_id, stop);
          draw();
        })
        .catch((reason) => {
          if (token === generation) error(reason);
        });
      position = -1;
      hasOrigin = false;
      choices.replaceChildren();
      suggestions.replaceChildren();
      status.textContent =
        "Saved route loaded. Each stop resolves against current material when followed.";
      draw();
      host.hidden = false;
    },
    stops: () => structuredClone(stops),
  };
}
