import type { ComparisonSide, GardenApi } from "./api";
export interface ComparisonSelection {
  sourceId: string;
  title: string;
  lineStart: number;
  draftLine: string;
  signature: string | null;
  response: ComparisonSide | null;
  error: string | null;
  scrollTop: number;
}
export interface ComparisonViewState {
  selections: [ComparisonSelection | null, ComparisonSelection | null];
}
export const newComparisonView = (): ComparisonViewState => ({
  selections: [null, null],
});
export const comparisonSelection = (
  sourceId: string,
  title: string,
): ComparisonSelection => ({
  sourceId,
  title,
  lineStart: 1,
  draftLine: "1",
  signature: null,
  response: null,
  error: null,
  scrollTop: 0,
});
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) => {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  return element;
};

/** Two bounded, temporary source selections. No comparison is written to the collection. */
export function renderComparison(
  host: HTMLElement,
  api: Pick<GardenApi, "comparisonSide" | "openComparisonOriginal">,
  state: ComparisonViewState,
  read: (sourceId: string) => void,
  close: () => void,
  report: (error: unknown) => void,
) {
  const panel = node("section");
  panel.setAttribute("aria-label", "Source comparison");
  host.replaceChildren(panel);
  state.selections.forEach((selected) => {
    if (selected) {
      selected.response = null;
      selected.error = null;
    }
  });
  let focusedInitially = false;
  const sequence = [0, 0];
  const busy = [false, false];
  const request = (index: number, fresh = false) => {
    const selected = state.selections[index]!;
    return {
      source_id: selected.sourceId,
      line_start: selected.lineStart,
      expected_signature: fresh ? null : selected.signature,
    };
  };
  async function load(index: number, fresh = false) {
    const selected = state.selections[index];
    if (!selected) return;
    const token = ++sequence[index];
    busy[index] = true;
    try {
      const response = await api.comparisonSide!(request(index, fresh));
      if (
        !panel.isConnected ||
        sequence[index] !== token ||
        state.selections[index] !== selected
      )
        return;
      const changed =
        JSON.stringify(response) !== JSON.stringify(selected.response);
      selected.response = response;
      selected.error = null;
      if (response.original_name) selected.title = response.title;
      if (response.status === "fresh" || selected.signature === null)
        selected.signature = response.signature;
      if (changed) draw();
    } catch (error) {
      if (panel.isConnected && sequence[index] === token) {
        selected.response = null;
        selected.error = String(error);
        draw();
        report(error);
      }
    } finally {
      if (sequence[index] === token) busy[index] = false;
    }
  }
  function button(label: string, action: () => void, id: string) {
    const element = node("button", label);
    element.type = "button";
    element.dataset.focusId = id;
    element.addEventListener("click", action);
    return element;
  }
  function draw() {
    if (!panel.isConnected) return;
    const scroll = host.scrollTop;
    const focus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement.dataset.focusId
        : undefined;
    const heading = node("h1", "Compare sources");
    heading.tabIndex = -1;
    panel.replaceChildren(
      heading,
      node(
        "p",
        "Read the retained passages with their qualifications. Open each source context for its current knowledge and surrounding evidence.",
      ),
      button("Close comparison", close, "comparison-close"),
    );
    if (state.selections[0]?.sourceId === state.selections[1]?.sourceId)
      panel.append(
        node(
          "p",
          "Both selections refer to the same retained source; choose different passage lines to compare its contexts.",
        ),
      );
    const sides = node("div");
    sides.className = "comparison-sides";
    state.selections.forEach((selected, index) => {
      const position = index === 0 ? "Left" : "Right";
      const side = node("section");
      side.setAttribute("aria-label", `${position} source comparison`);
      sides.append(side);
      if (!selected) {
        side.append(node("p", "Choose a source from its reading page."));
        return;
      }
      const response = selected.response;
      side.append(
        node("h2", response?.original_name ? response.title : selected.title),
      );
      if (!response) {
        side.append(
          node(
            "p",
            selected.error
              ? `Current passage unavailable: ${selected.error}`
              : "Loading current source passage…",
          ),
          button(
            `Retry ${position.toLowerCase()} selection`,
            () => {
              void load(index);
            },
            `comparison-retry-${index}`,
          ),
          button(
            `Read ${position.toLowerCase()} source context`,
            () => read(selected.sourceId),
            `comparison-context-${index}`,
          ),
        );
        return;
      }
      side.append(node("p", response.detail));
      const form = node("form");
      const label = node("label", `${position} first line `);
      const line = node("input");
      line.type = "number";
      line.min = "1";
      line.step = "1";
      line.value = selected.draftLine;
      line.dataset.focusId = `comparison-line-${index}`;
      line.addEventListener("input", () => {
        selected.draftLine = line.value;
      });
      label.append(line);
      const show = node("button", "Show passage");
      show.type = "submit";
      show.dataset.focusId = `comparison-show-${index}`;
      form.append(label, show);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const value = Number(selected.draftLine);
        if (!Number.isSafeInteger(value) || value < 1) {
          report(new Error("Choose a positive source line."));
          return;
        }
        selected.lineStart = value;
        selected.scrollTop = 0;
        void load(index);
      });
      side.append(form);
      if (response.passage) {
        const passage = response.passage;
        side.append(
          node(
            "p",
            `${response.format.toUpperCase()} · ${response.processing_status} · ${passage.offset_basis === "retained_original_text" ? "Original source" : "Extracted source projection"} lines ${passage.line_start}–${passage.line_end}`,
          ),
        );
        const quote = node("pre", passage.text);
        quote.tabIndex = 0;
        quote.dataset.focusId = `comparison-passage-${index}`;
        quote.addEventListener("scroll", () => {
          selected.scrollTop = quote.scrollTop;
        });
        quote.setAttribute("aria-label", `${position} source passage`);
        side.append(quote);
        quote.scrollTop = selected.scrollTop;
        if (passage.truncated)
          side.append(
            node(
              "p",
              "This bounded excerpt is shortened; open the original for full context.",
            ),
          );
        if (passage.next_line !== null)
          side.append(
            button(
              `Next ${position.toLowerCase()} passage`,
              () => {
                selected.lineStart = passage.next_line!;
                selected.scrollTop = 0;
                selected.draftLine = String(passage.next_line);
                void load(index);
              },
              `comparison-next-${index}`,
            ),
          );
      } else {
        const status = node(
          "p",
          response.status === "stale"
            ? "Selection changed; its earlier passage has been cleared."
            : "No source passage is available at this location.",
        );
        status.setAttribute("role", "status");
        side.append(status);
      }
      const original = button(
        `Open ${position.toLowerCase()} original`,
        () => {
          void api.openComparisonOriginal!(request(index)).catch((error) => {
            if (panel.isConnected) {
              report(error);
              void load(index);
            }
          });
        },
        `comparison-original-${index}`,
      );
      original.disabled =
        !api.openComparisonOriginal ||
        !response.asset ||
        response.status === "stale";
      side.append(
        button(
          `Read ${position.toLowerCase()} source context`,
          () => read(selected.sourceId),
          `comparison-context-${index}`,
        ),
        original,
        button(
          `Refresh ${position.toLowerCase()} selection`,
          () => {
            void load(index, true);
          },
          `comparison-refresh-${index}`,
        ),
      );
      for (const gap of response.context_gaps) side.append(node("p", gap));
      const details = node("details");
      details.append(
        node("summary", "Source identity and coverage"),
        node(
          "p",
          `${response.original_name} · ${response.origin ?? "Acquisition location unavailable"}`,
        ),
        node(
          "p",
          `Source ${response.source_id} · retained version ${response.source_version_id ?? "legacy original"}`,
        ),
      );
      if (response.coverage.length) {
        const list = node("ul");
        for (const part of response.coverage)
          list.append(
            node(
              "li",
              `${part.scope.replaceAll("_", " ")} · ${part.status}: ${part.detail}`,
            ),
          );
        details.append(list);
      } else
        details.append(
          node(
            "p",
            "No structured extraction coverage was recorded; read the source qualifications and processing state.",
          ),
        );
      side.append(details);
    });
    panel.append(sides);
    host.scrollTop = scroll;
    if (!focusedInitially) {
      heading.focus({ preventScroll: true });
      focusedInitially = true;
    }
    if (focus)
      panel
        .querySelector<HTMLElement>(`[data-focus-id="${CSS.escape(focus)}"]`)
        ?.focus({ preventScroll: true });
  }
  draw();
  const ready = Promise.all(
    state.selections.map((selected, index) =>
      selected ? load(index) : Promise.resolve(),
    ),
  );
  return {
    ready,
    refresh() {
      state.selections.forEach((selected, index) => {
        if (selected && !busy[index]) void load(index);
      });
    },
  };
}
