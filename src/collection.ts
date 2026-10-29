import type {
  GardenApi,
  CollectionAxis,
  CollectionGroup,
  CollectionGroups,
  CollectionItem,
  CollectionMembers,
} from "./api";
export interface CollectionViewState {
  axis: CollectionAxis;
  query: string;
  draftQuery: string;
  offset: number;
  presentation: "reading" | "research" | "canvas";
  result: CollectionGroups | null;
  expanded: Record<
    string,
    { group: CollectionGroup; offset: number; members: CollectionMembers }
  >;
  selected: CollectionItem | null;
}
export function newCollectionView(): CollectionViewState {
  return {
    axis: "topic",
    query: "",
    draftQuery: "",
    offset: 0,
    presentation: "reading",
    result: null,
    expanded: {},
    selected: null,
  };
}
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) => {
  const value = document.createElement(tag);
  if (text !== undefined) value.textContent = text;
  return value;
};

/** Only current bounded group/member windows live here; page content loads on selection. */
export function renderCollection(
  host: HTMLElement,
  api: Pick<GardenApi, "collectionGroups" | "collectionMembers">,
  state: CollectionViewState,
  read: (item: CollectionItem) => void,
  report: (error: unknown) => void,
) {
  const panel = node("section");
  panel.setAttribute("aria-label", "Collection overview");
  host.replaceChildren(panel);
  let generation = 0;
  let scope = JSON.stringify([state.axis, state.query, state.offset]);
  let sequence = 0;
  const waiting = new Set<string>();
  const requests = new Map<string, number>();
  const wanted = new Map(
    Object.values(state.expanded).map((value) => [
      value.group.group_id,
      { group: value.group, offset: value.offset },
    ]),
  );
  function collapse(groupId: string) {
    requests.delete(groupId);
    waiting.delete(groupId);
    wanted.delete(groupId);
    delete state.expanded[groupId];
    draw();
  }
  async function groups(offset: number) {
    const nextScope = JSON.stringify([state.axis, state.query, offset]);
    const scopeChanged = nextScope !== scope;
    if (scopeChanged) {
      scope = nextScope;
      requests.clear();
      waiting.clear();
      wanted.clear();
    }
    const current = ++generation;
    try {
      const result = await api.collectionGroups!({
        axis: state.axis,
        query: state.query,
        offset,
      });
      if (!panel.isConnected || current !== generation) return false;
      const unchanged =
        state.result?.revision === result.revision &&
        state.offset === offset &&
        JSON.stringify(state.result.groups) === JSON.stringify(result.groups);
      if (unchanged && !scopeChanged) return true;
      const priorRevision = state.result?.revision;
      const reopen =
        !scopeChanged && state.offset === offset ? [...wanted.values()] : [];
      requests.clear();
      waiting.clear();
      wanted.clear();
      state.expanded = {};
      state.offset = offset;
      state.result = result;
      draw();
      await Promise.all(
        reopen.map((previous) =>
          members(
            result.groups.find((g) => g.group_id === previous.group.group_id) ??
              previous.group,
            result.revision === priorRevision ? previous.offset : 0,
          ),
        ),
      );
      return true;
    } catch (error) {
      if (panel.isConnected && current === generation) report(error);
      return false;
    }
  }
  async function members(group: CollectionGroup, offset = 0) {
    if (waiting.has(group.group_id)) return;
    waiting.add(group.group_id);
    wanted.set(group.group_id, { group, offset });
    const token = ++sequence;
    requests.set(group.group_id, token);
    const requestScope = scope;
    const current = () =>
      panel.isConnected &&
      requestScope === scope &&
      requests.get(group.group_id) === token &&
      wanted.has(group.group_id);
    const revision = state.result?.revision ?? null;
    try {
      const result = await api.collectionMembers!({
        group_id: group.group_id,
        offset,
        revision,
      });
      if (!current()) return;
      if (result.changed) {
        waiting.delete(group.group_id);
        await groups(state.offset);
        if (current()) await members(group, 0);
        return;
      }
      state.expanded[group.group_id] = { group, offset, members: result };
      draw();
    } catch (error) {
      if (current()) report(error);
    } finally {
      if (requests.get(group.group_id) === token)
        waiting.delete(group.group_id);
    }
  }
  const open = (item: CollectionItem) => {
    state.selected = item;
    read(item);
  };
  function button(text: string, action: () => void, id: string) {
    const value = node("button", text);
    value.type = "button";
    value.dataset.focusId = id;
    value.addEventListener("click", action);
    return value;
  }
  function draw() {
    if (!panel.isConnected) return;
    const scroll = host.scrollTop;
    const focus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement.dataset.focusId
        : undefined;
    panel.className = `collection-overview collection-${state.presentation}`;
    panel.replaceChildren(
      node("h1", "Collection"),
      node(
        "p",
        "Topics organize current knowledge subjects and supported tags. Shared source support groups material without asserting a new relationship. Unclassified material remains available.",
      ),
    );
    const controls = node("form");
    const axisLabel = node("label", "Group by ");
    const axis = node("select");
    axis.setAttribute("aria-label", "Group collection by");
    axis.dataset.focusId = "collection-axis";
    for (const [value, label] of [
      ["topic", "Topic"],
      ["person", "Person"],
      ["place", "Place"],
      ["date", "Event date"],
      ["format", "Source format"],
    ]) {
      const option = node("option", label);
      option.value = value;
      axis.append(option);
    }
    axis.value = state.axis;
    axis.addEventListener("change", () => {
      state.axis = axis.value as CollectionAxis;
      state.expanded = {};
      void groups(0);
    });
    axisLabel.append(axis);
    const filterLabel = node("label", "Filter groups ");
    const filter = node("input");
    filter.type = "search";
    filter.value = state.draftQuery;
    filter.addEventListener("input", () => {
      state.draftQuery = filter.value;
    });
    filter.dataset.focusId = "collection-filter";
    filterLabel.append(filter);
    controls.addEventListener("submit", (event) => {
      event.preventDefault();
      state.query = filter.value;
      state.expanded = {};
      void groups(0);
    });
    const submit = node("button", "Apply group filter");
    submit.type = "submit";
    submit.dataset.focusId = "collection-apply-filter";
    const modeLabel = node("label", "Presentation ");
    const mode = node("select");
    mode.setAttribute("aria-label", "Collection presentation");
    mode.dataset.focusId = "collection-presentation";
    for (const value of ["reading", "research", "canvas"]) {
      const option = node("option", value[0].toUpperCase() + value.slice(1));
      option.value = value;
      mode.append(option);
    }
    mode.value = state.presentation;
    mode.addEventListener("change", () => {
      state.draftQuery = filter.value;
      state.presentation = mode.value as CollectionViewState["presentation"];
      draw();
    });
    modeLabel.append(mode);
    controls.append(
      axisLabel,
      filterLabel,
      submit,
      modeLabel,
      button(
        "Refresh collection",
        () => {
          void groups(state.offset);
        },
        "collection-refresh",
      ),
    );
    panel.append(controls);
    if (state.selected) {
      const selected = state.selected;
      const box = node("aside");
      box.setAttribute("aria-label", "Selected collection item");
      box.append(
        button(
          `Read selected: ${selected.title}`,
          () => open(selected),
          "collection-selected",
        ),
        node(
          "p",
          `${selected.kind} · ${selected.processing_status}. Selection remains available when outside this group window.`,
        ),
      );
      panel.append(box);
    }
    if (!state.result) {
      panel.append(node("p", "Loading collection groups…"));
      return;
    }
    if (state.presentation === "canvas") drawCanvas();
    const list = node("section");
    list.setAttribute("aria-label", "Structured collection groups");
    list.className = "collection-groups";
    for (const group of state.result.groups) {
      const entry = node("section");
      const expanded = state.expanded[group.group_id];
      const toggle = button(
        `${group.label} · ${group.members} items`,
        () => {
          if (state.expanded[group.group_id]) {
            collapse(group.group_id);
          } else {
            void members(group);
          }
        },
        `collection-group-${group.group_id}`,
      );
      toggle.setAttribute("aria-expanded", String(!!expanded));
      entry.append(toggle);
      if (expanded) {
        const items = node("ul");
        for (const item of expanded.members.items) {
          const row = node("li");
          const choice = button(
            item.title,
            () => open(item),
            `collection-item-${item.page_id}`,
          );
          choice.setAttribute(
            "aria-pressed",
            String(item.page_id === state.selected?.page_id),
          );
          row.append(
            choice,
            node(
              "span",
              ` · ${item.kind} · ${item.format} · ${item.processing_status}`,
            ),
          );
          if (item.page_type === "source")
            row.append(
              button(
                "Expand source neighborhood",
                () => {
                  void members({
                    group_id: `source:${item.source_id}`,
                    label: item.title,
                    members: 0,
                  });
                },
                `collection-neighborhood-${item.page_id}`,
              ),
            );
          items.append(row);
        }
        entry.append(items);
        appendMemberPages(
          entry,
          expanded.group,
          expanded.offset,
          expanded.members,
        );
      }
      list.append(entry);
    }
    panel.append(list);
    for (const expanded of Object.values(state.expanded).filter(
      (value) =>
        value.group.group_id.startsWith("source:") ||
        !state.result?.groups.some((g) => g.group_id === value.group.group_id),
    )) {
      const neighborhood = node("section");
      neighborhood.setAttribute(
        "aria-label",
        `Expanded scope: ${expanded.group.label}`,
      );
      neighborhood.append(
        node("h2", expanded.group.label),
        button(
          "Collapse expanded scope",
          () => collapse(expanded.group.group_id),
          `collection-close-${expanded.group.group_id}`,
        ),
        node(
          "p",
          expanded.group.group_id.startsWith("source:")
            ? "Pages supported by this retained source; open each page for its evidence and qualifications."
            : "Saved expanded scope outside this group page; membership is evaluated against current knowledge.",
        ),
      );
      for (const item of expanded.members.items)
        neighborhood.append(
          button(
            item.title,
            () => open(item),
            `collection-neighbor-${item.page_id}`,
          ),
        );
      appendMemberPages(
        neighborhood,
        expanded.group,
        expanded.offset,
        expanded.members,
      );
      panel.append(neighborhood);
    }
    const pagination = node("nav");
    pagination.setAttribute("aria-label", "Collection group pages");
    pagination.append(
      node(
        "span",
        `${state.offset + 1}–${state.offset + state.result.groups.length} groups`,
      ),
    );
    if (state.offset > 0)
      pagination.append(
        button(
          "Previous groups",
          () => {
            void groups(Math.max(0, state.offset - 50));
          },
          "collection-previous",
        ),
      );
    if (state.result.next_offset !== null)
      pagination.append(
        button(
          "Next groups",
          () => {
            void groups(state.result!.next_offset!);
          },
          "collection-next",
        ),
      );
    panel.append(pagination);
    if (!state.result.groups.length)
      panel.append(
        node(
          "p",
          "No matching groups. Change the filter or use Sources to reach the collection.",
        ),
      );
    host.scrollTop = scroll;
    if (focus)
      panel
        .querySelector<HTMLElement>(`[data-focus-id="${CSS.escape(focus)}"]`)
        ?.focus({ preventScroll: true });
  }
  function appendMemberPages(
    container: HTMLElement,
    group: CollectionGroup,
    offset: number,
    result: CollectionMembers,
  ) {
    if (offset > 0)
      container.append(
        button(
          `Previous members of ${group.label}`,
          () => {
            void members(group, Math.max(0, offset - 50));
          },
          `collection-members-prev-${group.group_id}`,
        ),
      );
    if (result.next_offset !== null)
      container.append(
        button(
          `More members of ${group.label}`,
          () => {
            void members(group, result.next_offset!);
          },
          `collection-members-next-${group.group_id}`,
        ),
      );
  }
  function drawCanvas() {
    const figure = node("figure");
    figure.className = "collection-canvas";
    figure.append(
      node(
        "figcaption",
        "Bounded canvas window. Use the equivalent structured groups below to expand, continue and read every item. Lines indicate group membership, not a stronger relationship.",
      ),
    );
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute(
      "viewBox",
      `0 0 900 ${Math.max(160, Math.ceil(state.result!.groups.length / 5) * 90)}`,
    );
    svg.setAttribute("aria-hidden", "true");
    state.result!.groups.forEach((group, index) => {
      const x = (index % 5) * 180 + 8,
        y = Math.floor(index / 5) * 90 + 8;
      const cell = document.createElementNS(svg.namespaceURI, "g");
      const rect = document.createElementNS(svg.namespaceURI, "rect");
      rect.setAttribute("x", String(x));
      rect.setAttribute("y", String(y));
      rect.setAttribute("width", "165");
      rect.setAttribute("height", "65");
      rect.setAttribute("rx", "10");
      rect.setAttribute(
        "fill",
        state.expanded[group.group_id] ? "#d9e6da" : "#f1eee6",
      );
      rect.setAttribute("stroke", "#355844");
      const label = document.createElementNS(svg.namespaceURI, "text");
      label.setAttribute("x", String(x + 10));
      label.setAttribute("y", String(y + 28));
      label.textContent =
        group.label.length > 19 ? group.label.slice(0, 18) + "…" : group.label;
      const title = document.createElementNS(svg.namespaceURI, "title");
      title.textContent = `${group.label} · ${group.members} items`;
      cell.append(title, rect, label);
      cell.addEventListener("click", () => {
        void members(group);
      });
      svg.append(cell);
    });
    figure.append(svg);
    const neighborhood = Object.values(state.expanded).at(-1);
    if (neighborhood) {
      figure.append(node("h2", `Neighborhood: ${neighborhood.group.label}`));
      const graph = document.createElementNS(svg.namespaceURI, "svg");
      graph.setAttribute(
        "viewBox",
        `0 0 900 ${100 + Math.ceil(neighborhood.members.items.length / 4) * 80}`,
      );
      graph.setAttribute("aria-hidden", "true");
      const center = document.createElementNS(svg.namespaceURI, "text");
      center.setAttribute("x", "450");
      center.setAttribute("y", "25");
      center.setAttribute("text-anchor", "middle");
      center.textContent = neighborhood.group.label;
      graph.append(center);
      neighborhood.members.items.forEach((item, index) => {
        const x = (index % 4) * 225 + 8,
          y = Math.floor(index / 4) * 80 + 80;
        const line = document.createElementNS(svg.namespaceURI, "line");
        line.setAttribute("x1", "450");
        line.setAttribute("y1", "35");
        line.setAttribute("x2", String(x + 100));
        line.setAttribute("y2", String(y));
        line.setAttribute("stroke", "#a2b2a5");
        line.setAttribute("stroke-dasharray", "3 3");
        graph.append(line);
        const cell = document.createElementNS(svg.namespaceURI, "g");
        const rect = document.createElementNS(svg.namespaceURI, "rect");
        rect.setAttribute("x", String(x));
        rect.setAttribute("y", String(y));
        rect.setAttribute("width", "207");
        rect.setAttribute("height", "54");
        rect.setAttribute("rx", "8");
        rect.setAttribute(
          "fill",
          item.page_id === state.selected?.page_id ? "#c3dbc8" : "#f1eee6",
        );
        rect.setAttribute("stroke", "#355844");
        const label = document.createElementNS(svg.namespaceURI, "text");
        label.setAttribute("x", String(x + 10));
        label.setAttribute("y", String(y + 30));
        label.textContent =
          item.title.length > 23 ? item.title.slice(0, 22) + "…" : item.title;
        const title = document.createElementNS(svg.namespaceURI, "title");
        title.textContent = item.title;
        cell.append(title, rect, label);
        cell.addEventListener("click", () => open(item));
        graph.append(cell);
      });
      figure.append(graph);
    }
    panel.append(figure);
  }
  draw();
  const ready = groups(state.offset);
  return { ready, refresh: () => groups(state.offset) };
}
