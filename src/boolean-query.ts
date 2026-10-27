import type {
  BooleanExpression,
  BooleanQueryRequest,
  BooleanQueryResult,
  GardenApi,
  PathExploreDiagnostic,
  QueryConstraint,
} from "./api";

export const emptyConstraint = (): QueryConstraint => ({
  date_from: null,
  date_to: null,
  format: null,
  processing_status: null,
});
export interface QueryView {
  request: BooleanQueryRequest;
  results: BooleanQueryResult[];
  diagnostics: PathExploreDiagnostic[];
  cursor: string | null;
  revision: string | null;
  complete: boolean;
  selected: string | null;
  stale: boolean;
  scrollTop: number;
}
export function newQuery(start: string): QueryView {
  return {
    request: {
      expression: {
        op: "and",
        children: [
          {
            op: "path",
            start_page_id: start,
            hops: [{ kind: "", direction: "with_arrow" }],
            endpoint: emptyConstraint(),
            intermediate: emptyConstraint(),
          },
        ],
      },
      returned: emptyConstraint(),
      continuation: null,
      work_budget: 50,
      page_size: 50,
    },
    results: [],
    diagnostics: [],
    cursor: null,
    revision: null,
    complete: false,
    selected: null,
    stale: false,
    scrollTop: 0,
  };
}
function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text = "",
): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag);
  result.textContent = text;
  return result;
}
export function renderBooleanQuery(
  host: HTMLElement,
  api: GardenApi,
  state: QueryView,
  callbacks: {
    active(): boolean;
    navigate(result: BooleanQueryResult): void;
    error(error: unknown): void;
  },
) {
  let generation = 0;
  let running = false;
  const panel = node("section");
  panel.className = "boolean-query";
  panel.setAttribute("aria-label", "Boolean query builder");
  const controls = node("div");
  const editors = new Map<BooleanExpression, HTMLElement>();
  const representation = node("pre");
  representation.setAttribute("aria-label", "Visible Boolean query");
  representation.setAttribute("aria-live", "polite");
  const output = node("div");
  const status = node("p");
  status.setAttribute("role", "status");
  panel.append(
    node("h1", "Boolean relationship query"),
    node(
      "p",
      "NOT removes pages from the known indexed set. It does not prove that a real-world relationship is false. Dates are event dates; format and processing status describe the originating source.",
    ),
    controls,
    representation,
    status,
    output,
  );
  host.replaceChildren(panel);
  function button(text: string, action: () => void) {
    const b = node("button", text);
    b.type = "button";
    b.addEventListener("click", action);
    return b;
  }
  function field(
    parent: HTMLElement,
    title: string,
    value: string,
    change: (s: string) => void,
    options?: string[],
    type = "text",
  ) {
    const label = node("label", title);
    const input = options ? node("select") : node("input");
    if (input instanceof HTMLSelectElement)
      for (const option of options ?? []) {
        const labels: Record<string, string> = {
          and: "AND",
          or: "OR",
          with_arrow: "Follow stored arrow",
          against_arrow: "Traverse against stored arrow",
          endpoint: "Endpoint (final hop)",
          intermediate: "Intermediate nodes",
        };
        const o = node("option", labels[option] ?? (option || "Any"));
        o.value = option;
        input.append(o);
      }
    else input.type = type;
    input.value = value;
    input.addEventListener(
      input instanceof HTMLSelectElement ? "change" : "input",
      () => {
        change(input.value);
        changed();
      },
    );
    label.append(input);
    parent.append(label);
  }
  function constraintText(c: QueryConstraint): string {
    return (
      [
        c.date_from ? `event date ≥ ${c.date_from}` : "",
        c.date_to ? `event date ≤ ${c.date_to}` : "",
        c.format ? `source format = ${c.format}` : "",
        c.processing_status ? `processing status = ${c.processing_status}` : "",
      ]
        .filter(Boolean)
        .join(" AND ") || "Any"
    );
  }
  function expressionText(e: BooleanExpression): string {
    if (e.op === "and" || e.op === "or")
      return `(${e.children.map(expressionText).join(` ${e.op.toUpperCase()} `) || (e.op === "and" ? "All indexed pages" : "No pages")})`;
    if (e.op === "not") return `NOT (${expressionText(e.child)})`;
    if (e.op === "constraint")
      return `Returned page condition [${constraintText(e.constraint)}]`;
    return `Path from ${e.start_page_id}: ${e.hops.map((h) => (h.direction === "with_arrow" ? `—${h.kind || "Choose type"}→` : `←${h.kind || "Choose type"}—`)).join(" ")} [Endpoint: ${constraintText(e.endpoint)}; Intermediate nodes: ${constraintText(e.intermediate)}]`;
  }
  function showRepresentation() {
    representation.textContent =
      expressionText(state.request.expression) +
      "\nReturned page set: " +
      constraintText(state.request.returned);
  }

  function cancel() {
    generation++;
    running = false;
    if (state.cursor) void api.cancelQuery(state.cursor).catch(callbacks.error);
    state.cursor = null;
  }
  function changed() {
    cancel();
    state.results = [];
    state.diagnostics = [];
    state.revision = null;
    state.complete = false;
    state.stale = false;
    state.selected = null;
    showRepresentation();
    showResults();
    status.textContent =
      "Query changed. Run query to calculate current matches.";
  }
  function constraints(parent: HTMLElement, constraint: QueryConstraint) {
    field(
      parent,
      "Event date from",
      constraint.date_from ?? "",
      (s) => (constraint.date_from = s || null),
      undefined,
      "date",
    );
    field(
      parent,
      "Event date to",
      constraint.date_to ?? "",
      (s) => (constraint.date_to = s || null),
      undefined,
      "date",
    );
    field(
      parent,
      "Source format",
      constraint.format ?? "",
      (s) => (constraint.format = s || null),
    );
    field(
      parent,
      "Processing status",
      constraint.processing_status ?? "",
      (s) => (constraint.processing_status = s || null),
      [
        "",
        "pending",
        "processing",
        "complete",
        "failed",
        "unavailable",
        "update_pending",
        "update_failed",
        "identity_ambiguous",
        "extraction_incomplete",
        "source_unavailable",
      ],
    );
  }
  function editor(
    expression: BooleanExpression,
    replace: (e: BooleanExpression) => void,
    remove?: () => void,
  ): HTMLElement {
    const box = node("fieldset");
    editors.set(expression, box);
    box.append(
      node(
        "legend",
        expression.op === "path"
          ? "Typed path"
          : expression.op === "constraint"
            ? "Returned page condition"
            : "Boolean group",
      ),
    );
    if (remove) box.append(button("Remove clause", remove));
    box.append(
      button(expression.op === "not" ? "Remove NOT" : "Wrap in NOT", () => {
        const replacement =
          expression.op === "not"
            ? expression.child
            : ({ op: "not", child: expression } as const);
        replace(replacement);
        changed();
        drawControls(
          replacement,
          expression.op === "not" ? "Wrap in NOT" : "Remove NOT",
        );
      }),
    );
    if (expression.op === "and" || expression.op === "or") {
      field(
        box,
        "Group operator",
        expression.op,
        (s) => (expression.op = s as "and" | "or"),
        ["and", "or"],
      );
      expression.children.forEach((child, i) =>
        box.append(
          editor(
            child,
            (e) => (expression.children[i] = e),
            () => {
              expression.children.splice(i, 1);
              changed();
              drawControls(expression, "Add path clause");
            },
          ),
        ),
      );
      const add = (e: BooleanExpression) => {
        expression.children.push(e);
        changed();
        drawControls(e);
      };
      box.append(
        button("Add path clause", () =>
          add({
            op: "path",
            start_page_id: initialStart,
            hops: [{ kind: "", direction: "with_arrow" }],
            endpoint: emptyConstraint(),
            intermediate: emptyConstraint(),
          }),
        ),
        button("Add condition", () =>
          add({ op: "constraint", constraint: emptyConstraint() }),
        ),
        button("Add group", () => add({ op: "and", children: [] })),
      );
    } else if (expression.op === "not")
      box.append(editor(expression.child, (e) => (expression.child = e)));
    else if (expression.op === "constraint")
      constraints(box, expression.constraint);
    else if (expression.op === "path") {
      field(
        box,
        "Starting page ID",
        expression.start_page_id,
        (s) => (expression.start_page_id = s),
      );
      expression.hops.forEach((hop, i) => {
        const row = node("div");
        row.append(node("span", `Hop ${i + 1}`));
        field(row, "Relationship type", hop.kind, (s) => (hop.kind = s));
        field(
          row,
          "Hop direction",
          hop.direction,
          (s) => (hop.direction = s as typeof hop.direction),
          ["with_arrow", "against_arrow"],
        );
        row.append(
          button("Remove hop", () => {
            expression.hops.splice(i, 1);
            changed();
            drawControls(expression, "Add hop");
          }),
        );
        box.append(row);
      });
      box.append(
        button("Add hop", () => {
          expression.hops.push({ kind: "", direction: "with_arrow" });
          changed();
          drawControls(expression, "Add hop");
        }),
      );
      const scope = node("div");
      const fields = node("div");
      field(
        scope,
        "Constraint scope",
        "endpoint",
        (s) => {
          fields.replaceChildren();
          constraints(
            fields,
            s === "endpoint" ? expression.endpoint : expression.intermediate,
          );
        },
        ["endpoint", "intermediate"],
      );
      constraints(fields, expression.endpoint);
      box.append(scope, fields);
    }
    return box;
  }
  function findStart(e: BooleanExpression): string {
    return e.op === "path"
      ? e.start_page_id
      : e.op === "not"
        ? findStart(e.child)
        : e.op === "and" || e.op === "or"
          ? (e.children.map(findStart).find(Boolean) ?? "")
          : "";
  }
  const initialStart = findStart(state.request.expression);
  function drawControls(focus?: BooleanExpression, buttonLabel?: string) {
    editors.clear();
    const expression = editor(
      state.request.expression,
      (e) => (state.request.expression = e),
    );
    expression.setAttribute("role", "group");
    expression.setAttribute("aria-label", "Boolean query expression");
    const returned = node("fieldset");
    returned.append(node("legend", "Constraints on returned page set"));
    constraints(returned, state.request.returned);
    controls.replaceChildren(
      expression,
      returned,
      button("Run query", () => {
        cancel();
        state.results = [];
        state.diagnostics = [];
        state.complete = false;
        state.revision = null;
        state.stale = false;
        showResults();
        void run(null);
      }),
      button("Cancel query", () => {
        cancel();
        status.textContent =
          "Query canceled. Existing matches are a partial set.";
      }),
      button("Clear query", () => {
        state.request = newQuery(initialStart).request;
        state.request.expression = { op: "and", children: [] };
        changed();
        drawControls(undefined, "Clear query");
      }),
    );
    const scope = focus ? editors.get(focus) : controls;
    if (buttonLabel)
      [...(scope?.querySelectorAll<HTMLButtonElement>(":scope > button") ?? [])]
        .find((button) => button.textContent === buttonLabel)
        ?.focus();
    else if (focus)
      scope?.querySelector<HTMLElement>("input, select, button")?.focus();
  }
  async function inspect(
    result: BooleanQueryResult,
    detailToken: string,
    target: HTMLElement,
  ) {
    const version = generation;
    let cursor: string | null = null;
    const steps = [];
    do {
      const detail = await api.explorePathDetails({
        detail_token: detailToken,
        continuation: cursor,
        work_budget: 50,
      });
      if (version !== generation || !callbacks.active()) return;
      steps.push(...detail.steps);
      cursor = detail.next_cursor;
    } while (cursor);
    target.replaceChildren();
    for (const step of steps.reverse()) {
      const row = node("div");
      row.append(
        node(
          "p",
          `${step.from_title} → ${step.kind} → ${step.to_title} · ${step.traversal_direction}${step.qualifier ? " · " + step.qualifier : ""}`,
        ),
        node("blockquote", step.evidence_quote),
        node(
          "p",
          `${step.evidence_origin} · ${step.evidence_location} · ${step.evidence_qualifier ?? ""} · ${step.original_guidance} · ${step.source_is_current ? "Current source version" : "Retained earlier source version"}`,
        ),
      );
      row.append(
        button("Open supporting original", () => {
          state.selected = result.page_id;
          const operation = step.source_is_current
            ? api.openOriginalAsset(step.source_id, step.original_asset)
            : api.openOriginalVersion(
                step.source_id,
                step.source_version_id,
                step.original_asset,
              );
          void operation.catch(callbacks.error);
        }),
      );
      target.append(row);
    }
  }
  function showResults() {
    output.replaceChildren(
      node(
        "p",
        `${state.results.length} matching pages${state.complete ? " · complete" : " · partial"}.`,
      ),
    );
    for (const diagnostic of state.diagnostics) {
      output.append(
        node(
          "p",
          diagnostic.kind === "missing_support"
            ? `Relationship to ${diagnostic.endpoint_page_id} omitted because acquired supporting material is unavailable.`
            : `Relationship endpoint ${diagnostic.endpoint_page_id} is unavailable in the indexed set.`,
        ),
      );
    }
    for (const result of state.results) {
      const card = node("article");
      const open = button(`Open query result ${result.title}`, () => {
        state.selected = result.page_id;
        callbacks.navigate(result);
      });
      open.dataset.focusId = `query-result:${result.page_id}`;
      card.append(open);
      if (!result.paths.length)
        card.append(
          node(
            "p",
            "Matches indexed page conditions; no positive connecting path is asserted.",
          ),
        );
      for (const [i, path] of result.paths.entries()) {
        const details = node("div");
        card.append(
          button(`Inspect supporting path ${i + 1} for ${result.title}`, () => {
            void inspect(result, path.detail_token, details).catch(
              callbacks.error,
            );
          }),
          details,
        );
      }
      output.append(card);
    }
    if (state.cursor && !state.stale)
      output.append(
        button("Continue query", () => {
          void run(state.cursor);
        }),
      );
  }
  async function run(cursor: string | null): Promise<boolean> {
    const version = ++generation;
    running = true;
    status.textContent = "Computing indexed query…";
    try {
      const response = await api.queryPages({
        ...state.request,
        continuation: cursor,
      });
      if (version !== generation || !callbacks.active()) {
        if (response.next_cursor)
          void api.cancelQuery(response.next_cursor).catch(callbacks.error);
        return false;
      }
      if (state.revision && state.revision !== response.graph_revision)
        throw new Error(
          "Query cursor is stale because the collection changed.",
        );
      state.revision = response.graph_revision;
      state.cursor = response.next_cursor;
      state.complete = response.complete;
      state.results.push(...response.results);
      const diagnostics = new Map(
        state.diagnostics.map((d) => [`${d.kind}:${d.relationship_id}`, d]),
      );
      for (const diagnostic of response.diagnostics)
        diagnostics.set(
          `${diagnostic.kind}:${diagnostic.relationship_id}`,
          diagnostic,
        );
      state.diagnostics = [...diagnostics.values()];
      status.textContent = `${response.work_units} indexed work units in this batch. ${response.diagnostics.length} unavailable relationship(s) disclosed.`;
      showResults();
      return true;
    } catch (error) {
      if (version === generation && callbacks.active()) {
        if (/stale|expired/i.test(String(error))) {
          state.stale = true;
          state.results = [];
          state.diagnostics = [];
          state.cursor = null;
          state.revision = null;
          showResults();
          status.textContent =
            "Collection changed. Run query again for current matches.";
        } else callbacks.error(error);
      }
      return false;
    } finally {
      if (version === generation) running = false;
    }
  }
  drawControls();
  showRepresentation();
  showResults();
  if (state.stale)
    status.textContent =
      "Collection changed. Run query again for current matches.";
  host.scrollTop = state.scrollTop;
  if (state.selected)
    host
      .querySelector<HTMLElement>(
        `[data-focus-id="query-result:${CSS.escape(state.selected)}"]`,
      )
      ?.focus();
  else controls.querySelector<HTMLElement>("select, input")?.focus();
  return {
    run: () => run(null),
    invalidate() {
      if (!state.stale) {
        cancel();
        state.stale = true;
        state.results = [];
        state.diagnostics = [];
        state.complete = false;
        state.revision = null;
        showResults();
        status.textContent =
          "Collection changed. Run query again for current matches.";
      }
    },
    stop() {
      generation++;
      running = false;
    },
    get running() {
      return running;
    },
  };
}
