import type { GardenApi, SavedViews } from "./api";
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) => {
  const value = document.createElement(tag);
  if (text !== undefined) value.textContent = text;
  return value;
};
type ViewApi = Pick<GardenApi, "listSavedViews">;
/** Explicit organization only. Merely opening this panel never writes a definition. */
export function savedViewsPanel(
  host: HTMLElement,
  api: ViewApi,
  open: (id: string) => Promise<string | null>,
  report: (error: unknown) => void,
) {
  host.hidden = true;
  host.className = "saved-views";
  host.setAttribute("aria-label", "Saved views");
  const heading = node("h2", "Saved views");
  heading.tabIndex = -1;
  const close = button("Close saved views", () => {
    host.hidden = true;
  });
  const status = node("p");
  status.setAttribute("role", "status");
  const find = node("form");
  const queryLabel = node("label", "Find saved views ");
  const query = node("input");
  query.type = "search";
  queryLabel.append(query);
  const apply = node("button", "Find views");
  apply.type = "submit";
  find.append(queryLabel, apply);
  const list = node("div");
  const paging = node("div");
  const diagnostics = node("div");
  host.replaceChildren(heading, close, find, status, list, paging, diagnostics);
  let offset = 0,
    filter = "",
    sequence = 0,
    mutating = false;
  function button(text: string, action: () => void) {
    const control = node("button", text);
    control.type = "button";
    control.addEventListener("click", () => {
      control.focus();
      action();
    });
    return control;
  }
  function error(error: unknown) {
    status.textContent = String(error);
    report(error);
  }
  function setBusy(value: boolean) {
    for (const control of host.querySelectorAll<
      HTMLInputElement | HTMLButtonElement
    >("input,button"))
      control.disabled = value;
    close.disabled = false;
  }
  async function mutate(operation: () => Promise<void>) {
    if (mutating) return;
    mutating = true;
    setBusy(true);
    host.setAttribute("aria-busy", "true");
    try {
      await operation();
    } catch (reason) {
      error(reason);
    } finally {
      mutating = false;
      setBusy(false);
      host.removeAttribute("aria-busy");
    }
  }
  async function load() {
    const token = ++sequence;
    try {
      const result = await api.listSavedViews!(filter, offset);
      if (!host.isConnected || token !== sequence) return;
      draw(result);
    } catch (reason) {
      if (host.isConnected && token === sequence) error(reason);
    }
  }
  function draw(result: SavedViews) {
    list.replaceChildren();
    paging.replaceChildren();
    diagnostics.replaceChildren();
    if (!result.views.length)
      list.append(
        node(
          "p",
          "No saved views match. Unsaved exploration creates no permanent view.",
        ),
      );
    for (const view of result.views) {
      const row = node("section");
      row.append(node("h3", view.name));
      row.append(
        button(`Open saved view ${view.name}`, () => {
          void mutate(async () => {
            const opened = await open(view.view_id);
            if (opened !== null)
              status.textContent = `Opened ${opened} against current knowledge.`;
          });
        }),
      );
      list.append(row);
    }
    if (offset > 0)
      paging.append(
        button("Previous saved views", () => {
          offset = Math.max(0, offset - 50);
          void load();
        }),
      );
    if (result.next_offset !== null)
      paging.append(
        button("More saved views", () => {
          offset = result.next_offset!;
          void load();
        }),
      );
    for (const diagnostic of result.diagnostics) {
      const row = node("p", `${diagnostic.path}: ${diagnostic.detail}`);
      diagnostics.append(row);
    }
    if (mutating) setBusy(true);
  }
  find.addEventListener("submit", (event) => {
    event.preventDefault();
    filter = query.value.trim();
    offset = 0;
    void load();
  });
  return {
    show() {
      host.hidden = false;
      void load();
      heading.focus();
    },
    hide() {
      host.hidden = true;
    },
    toggle() {
      if (host.hidden) this.show();
      else this.hide();
    },
  };
}
