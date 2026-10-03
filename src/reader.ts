import DOMPurify from "dompurify";
import { marked } from "marked";
import type {
  AcquisitionMethod,
  GardenApi,
  KnowledgePage,
  PageResult,
  PageSearchRequest,
  PageSearchResults,
  SourcePage,
  SourceSummary,
} from "./api";

const LAST_SOURCE = "knowledge-garden:last-source";
const labels = {
  text_preserved: "Text preserved",
  unsupported: "Unsupported format",
  invalid_utf8: "Text could not be decoded",
  too_large: "Text exceeds preview limit",
};

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}

export async function mountReader(
  root: HTMLElement,
  api: GardenApi,
): Promise<() => void> {
  const toolbar = element("header");
  toolbar.className = "toolbar";
  const brand = element("span", "Knowledge Garden");
  brand.className = "brand";
  const backButton = element("button", "Back");
  backButton.hidden = true;
  const searchButton = element("button", "Search");
  const sourcesButton = element("button", "Sources");
  sourcesButton.setAttribute("aria-expanded", "false");
  const addButton = element("button", "Add source");
  addButton.className = "primary";
  toolbar.append(brand, backButton, searchButton, sourcesButton, addButton);
  const notice = element("p");
  notice.className = "notice";
  notice.setAttribute("role", "status");
  notice.setAttribute("aria-live", "polite");
  const sources = element("nav");
  sources.className = "sources";
  sources.setAttribute("aria-label", "Collection sources");
  sources.hidden = true;
  const main = element("main");
  main.className = "reader";
  const empty = element("section");
  empty.className = "empty";
  empty.append(
    element("h1", "A place for what you know"),
    element(
      "p",
      "Choose a text or Markdown file, or drop one anywhere in this window. Your original stays intact.",
    ),
  );
  main.append(empty);
  root.replaceChildren(toolbar, notice, sources, main);
  let page: SourcePage | null = null;
  let currentKnowledgePageId: string | null = null;
  let currentSourceId: string | null = null;
  let currentView: "empty" | "source" | "knowledge" | "search" = "empty";
  let currentSearch: PageSearchRequest | null = null;
  let currentSelectedPageId: string | null = null;
  let searchResults: PageSearchResults | null = null;
  type NavigationState =
    | {
        kind: "search";
        request: PageSearchRequest;
        selectedPageId: string | null;
        scrollTop: number;
        windowScroll: number;
      }
    | {
        kind: "source";
        sourceId: string;
        scrollTop: number;
        windowScroll: number;
        focusHref: string | null;
        focusText: string | null;
      }
    | {
        kind: "knowledge";
        pageId: string;
        scrollTop: number;
        windowScroll: number;
        focusHref: string | null;
        focusText: string | null;
      };
  const navigationHistory: NavigationState[] = [];
  let busy = false;
  let disposed = false;

  function message(text: string) {
    if (!disposed) notice.textContent = text;
  }
  function report(error: unknown) {
    message(error instanceof Error ? error.message : String(error));
  }

  function syncBackButton() {
    backButton.hidden = navigationHistory.length === 0;
  }

  function pushCurrentState() {
    const mainArticle = main.querySelector("article");
    if (currentView === "search" && currentSearch) {
      navigationHistory.push({
        kind: "search",
        request: { ...currentSearch, tags: [...currentSearch.tags] },
        selectedPageId: currentSelectedPageId,
        scrollTop: main.scrollTop,
        windowScroll: window.scrollY,
      });
    } else if (currentView === "source" && currentSourceId) {
      const activeLink = document.activeElement?.closest("a");
      navigationHistory.push({
        kind: "source",
        sourceId: currentSourceId,
        scrollTop: mainArticle?.scrollTop ?? 0,
        windowScroll: window.scrollY,
        focusHref: activeLink?.getAttribute("href") ?? null,
        focusText: activeLink?.textContent ?? null,
      });
    } else if (currentView === "knowledge" && currentKnowledgePageId) {
      const activeLink = document.activeElement?.closest("a");
      navigationHistory.push({
        kind: "knowledge",
        pageId: currentKnowledgePageId,
        scrollTop: mainArticle?.scrollTop ?? 0,
        windowScroll: window.scrollY,
        focusHref: activeLink?.getAttribute("href") ?? null,
        focusText: activeLink?.textContent ?? null,
      });
    }
    syncBackButton();
  }

  function showMarkdown(
    markdown: string,
    title: string,
    sourceId: string,
    originalAsset?: string,
  ) {
    if (disposed) return;
    const article = element("article");
    article.tabIndex = -1;
    article.setAttribute("aria-label", title);
    article.innerHTML = DOMPurify.sanitize(
      marked.parse(markdown.replace(/^---\n[\s\S]*?\n---\n/, ""), {
        async: false,
      }),
      {
        USE_PROFILES: { html: true },
        FORBID_TAGS: ["img", "video", "audio", "iframe", "style", "form"],
        FORBID_ATTR: ["style", "id", "name"],
      },
    );
    article.addEventListener("click", (event) => {
      const link = (event.target as Element).closest("a");
      if (!link) return;
      event.preventDefault();
      const href = link.getAttribute("href") ?? "";
      const knowledgeMatch = href.match(
        /(?:\.\.\/)?pages\/(page-[a-zA-Z0-9-]+)\.md/,
      );
      const sourceMatch = href.match(
        /(?:\.\.\/)?sources\/([a-f0-9]{64})\/index\.md/,
      );
      const originalMatch = href.match(
        /(?:\.\.\/)?sources\/([a-f0-9]{64})\/original(?:\.[a-zA-Z0-9]+)?/,
      );
      if (href === originalAsset || href === `knowledge-original:${sourceId}`) {
        void api.openOriginal(sourceId).catch(report);
      } else if (knowledgeMatch) {
        pushCurrentState();
        void api
          .openKnowledgePage(knowledgeMatch[1])
          .then(showKnowledgePage)
          .catch(report);
      } else if (sourceMatch) {
        pushCurrentState();
        void api
          .openSource(`source-${sourceMatch[1]}`)
          .then(showPage)
          .catch(report);
      } else if (originalMatch) {
        void api.openOriginal(`source-${originalMatch[1]}`).catch(report);
      } else {
        message("This reference is preserved in the Markdown page.");
      }
    });
    main.replaceChildren(article);
    sources.hidden = true;
    sourcesButton.setAttribute("aria-expanded", "false");
    try {
      localStorage.setItem(LAST_SOURCE, sourceId);
    } catch {
      /* Reading works without browser storage. */
    }
    article.focus();
    return article;
  }

  function showPage(next: SourcePage) {
    if (disposed) return;
    page = next;
    currentSourceId = next.info.source_id;
    currentKnowledgePageId = null;
    currentView = "source";
    const article = showMarkdown(
      next.body,
      next.info.title,
      next.info.source_id,
      next.info.asset,
    );
    if (!article) return;
    const details = element("details");
    details.className = "source-info";
    details.append(element("summary", "Source information"));
    const definition = element("dl");
    const rows = [
      ["Original", next.info.original_name],
      ["Processing", labels[next.info.extraction]],
      [
        "Knowledge",
        next.info.semantic_state === "complete"
          ? "Ready"
          : next.info.semantic_state === "unavailable"
            ? "Not available for this format"
            : next.info.semantic_state === "failed"
              ? "Processing failed"
              : next.info.semantic_state === "processing"
                ? "Semantic processing in progress"
                : next.info.semantic_error && next.info.semantic_retry_at
                  ? "Waiting to retry semantic processing"
                  : "Waiting for semantic processing",
      ],
      [
        "Coverage",
        next.info.semantic_state === "complete"
          ? "Supported semantic spans are published with their original evidence and qualifiers."
          : next.info.extraction_detail,
      ],
      ["Format", next.info.format || "No extension"],
      ["Size", `${next.info.bytes.toLocaleString()} bytes`],
      ["Source identity", next.info.source_id],
      ["Page identity", next.info.page_id],
    ];
    if (next.info.semantic_error)
      rows.push(["Semantic status", next.info.semantic_error]);
    for (const acquisition of next.info.acquisitions) {
      rows.push(["Acquired from", acquisition.path]);
      rows.push([
        "Received",
        new Date(Number(acquisition.received_at)).toLocaleString(),
      ]);
      rows.push([
        "Added by",
        acquisition.method === "picker" ? "File picker" : "File drop",
      ]);
    }
    for (const [label, value] of rows)
      definition.append(element("dt", label), element("dd", value));
    details.append(definition);
    main.append(details);
  }

  function showKnowledgePage(next: KnowledgePage) {
    page = null;
    currentKnowledgePageId = next.page_id;
    currentSourceId = next.source_id;
    currentView = "knowledge";
    const article = showMarkdown(next.markdown, next.title, next.source_id);
    if (!article) return;
    const returnLink = element("a", "Return to source page");
    returnLink.href = `../sources/${next.source_id.slice("source-".length)}/index.md`;
    article.prepend(returnLink);
  }

  function highlightQuote(quote: string | null | undefined) {
    if (!quote || disposed) return;
    const article = main.querySelector("article");
    if (!article) return;
    const walker = document.createTreeWalker(article, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      const text = node.textContent ?? "";
      const index = text.indexOf(quote);
      if (index < 0 || !node.parentElement) continue;
      const mark = element("mark", quote);
      const before = document.createTextNode(text.slice(0, index));
      const after = document.createTextNode(text.slice(index + quote.length));
      node.parentElement.insertBefore(before, node);
      node.parentElement.insertBefore(mark, node);
      node.parentElement.insertBefore(after, node);
      node.parentElement.removeChild(node);
      mark.scrollIntoView?.({ block: "center" });
      return;
    }
  }

  function renderSearch(
    results: PageSearchResults,
    request: PageSearchRequest,
  ) {
    searchResults = results;
    currentSearch = { ...request, tags: [...request.tags] };
    currentSelectedPageId = null;
    currentView = "search";
    currentSourceId = null;
    currentKnowledgePageId = null;
    sources.hidden = true;
    sourcesButton.setAttribute("aria-expanded", "false");
    const panel = element("section");
    panel.className = "search-panel";
    panel.setAttribute("aria-label", "Search pages");
    const heading = element("h1", "Search your garden");
    const form = element("form");
    form.className = "search-form";
    const queryLabel = element("label", "Words or title");
    const query = element("input");
    query.type = "search";
    query.name = "query";
    query.value = request.query;
    query.setAttribute("aria-label", "Words or title");
    queryLabel.append(query);
    form.append(queryLabel);

    const dates = element("div");
    dates.className = "search-row";
    for (const [name, labelText, value] of [
      ["date_from", "From date", request.date_from ?? ""],
      ["date_to", "To date", request.date_to ?? ""],
    ]) {
      const label = element("label", labelText);
      const input = element("input");
      input.type = "date";
      input.name = name;
      input.value = value;
      input.setAttribute("aria-label", labelText);
      label.append(input);
      dates.append(label);
    }
    form.append(dates);

    const selects = element("div");
    selects.className = "search-row";
    for (const [name, labelText, options, selected] of [
      ["format", "Format", results.available_formats, request.format],
      [
        "processing_status",
        "Processing status",
        results.available_statuses,
        request.processing_status,
      ],
    ] as const) {
      const label = element("label", labelText);
      const select = element("select");
      select.name = name;
      select.setAttribute("aria-label", labelText);
      const any = element("option", "Any");
      any.value = "";
      select.append(any);
      for (const optionValue of options) {
        const option = element("option", optionValue);
        option.value = optionValue;
        select.append(option);
      }
      select.value = selected ?? "";
      label.append(select);
      selects.append(label);
    }
    form.append(selects);

    if (results.available_tags.length) {
      const fieldset = element("fieldset");
      fieldset.className = "tag-filters";
      fieldset.append(element("legend", "Tags"));
      for (const tag of results.available_tags) {
        const label = element("label", `#${tag}`);
        const input = element("input");
        input.type = "checkbox";
        input.name = "tag";
        input.value = tag;
        input.checked = request.tags.includes(tag);
        input.setAttribute("aria-label", `Tag ${tag}`);
        label.prepend(input);
        fieldset.append(label);
      }
      form.append(fieldset);
    }
    const submit = element("button", "Search");
    submit.type = "submit";
    submit.className = "primary";
    form.append(submit);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const data = new FormData(form);
      const next: PageSearchRequest = {
        query: String(data.get("query") ?? "").trim(),
        tags: data.getAll("tag").map(String),
        date_from: String(data.get("date_from") ?? "") || null,
        date_to: String(data.get("date_to") ?? "") || null,
        format: String(data.get("format") ?? "") || null,
        processing_status: String(data.get("processing_status") ?? "") || null,
        offset: 0,
      };
      void runSearch(next, false).catch(report);
    });
    panel.append(heading, form);

    const active = element("div");
    active.className = "active-filters";
    active.setAttribute("aria-label", "Active filters");
    const filters: [string, () => PageSearchRequest][] = [];
    request.tags.forEach((tag) =>
      filters.push([
        `#${tag}`,
        () => ({
          ...request,
          tags: request.tags.filter((x) => x !== tag),
          offset: 0,
        }),
      ]),
    );
    if (request.query)
      filters.push([
        `Words: ${request.query}`,
        () => ({ ...request, query: "", offset: 0 }),
      ]);
    if (request.date_from)
      filters.push([
        `From ${request.date_from}`,
        () => ({ ...request, date_from: null, offset: 0 }),
      ]);
    if (request.date_to)
      filters.push([
        `To ${request.date_to}`,
        () => ({ ...request, date_to: null, offset: 0 }),
      ]);
    if (request.format)
      filters.push([
        `Format: ${request.format}`,
        () => ({ ...request, format: null, offset: 0 }),
      ]);
    if (request.processing_status)
      filters.push([
        `Status: ${request.processing_status}`,
        () => ({ ...request, processing_status: null, offset: 0 }),
      ]);
    for (const [name, clear] of filters) {
      const chip = element("button", `${name} ×`);
      chip.type = "button";
      chip.setAttribute("aria-label", `Remove ${name} filter`);
      chip.addEventListener(
        "click",
        () => void runSearch(clear(), false).catch(report),
      );
      active.append(chip);
    }
    if (filters.length) panel.append(active);

    const list = element("div");
    list.className = "search-results";
    list.setAttribute("aria-live", "polite");
    if (!results.pages.length)
      list.append(
        element(
          "p",
          "No matching pages. Try removing a filter or changing your words.",
        ),
      );
    for (const result of results.pages) {
      const card = element("article");
      card.className = "search-result";
      const open = element("button", result.title);
      open.className = "result-title";
      open.type = "button";
      open.dataset.pageId = result.page_id;
      open.addEventListener("click", () => void openResult(result));
      const meta = element(
        "p",
        `${result.page_type === "source" ? "Source" : result.kind} · ${result.format || "no extension"} · ${result.processing_status}${result.event_date ? ` · ${result.event_date}` : ""}`,
      );
      meta.className = "result-meta";
      const excerpt = element("p", result.excerpt || "No excerpt available.");
      excerpt.className = "result-excerpt";
      card.append(open, meta, excerpt);
      if (result.tags.length) {
        const tagLine = element(
          "p",
          result.tags.map((tag) => `#${tag}`).join(" "),
        );
        tagLine.className = "result-tags";
        card.append(tagLine);
      }
      list.append(card);
    }
    panel.append(list);
    const pageControls = element("nav");
    pageControls.className = "search-pagination";
    pageControls.setAttribute("aria-label", "Search result pages");
    if (request.offset > 0) {
      const previous = element("button", "Previous results");
      previous.addEventListener(
        "click",
        () =>
          void runSearch(
            { ...request, offset: Math.max(0, request.offset - 50) },
            false,
          ).catch(report),
      );
      pageControls.append(previous);
    }
    if (results.next_offset !== null) {
      const next = element("button", "Next results");
      next.addEventListener(
        "click",
        () =>
          void runSearch(
            { ...request, offset: results.next_offset! },
            false,
          ).catch(report),
      );
      pageControls.append(next);
    }
    if (pageControls.childElementCount) panel.append(pageControls);
    main.replaceChildren(panel);
    main.scrollTop = 0;
  }

  async function runSearch(request: PageSearchRequest, push: boolean) {
    if (push) pushCurrentState();
    message("Searching pages…");
    const results = await api.searchPages(request);
    if (disposed) return;
    renderSearch(results, request);
    message(
      `${results.pages.length} matching ${results.pages.length === 1 ? "page" : "pages"}.`,
    );
    syncBackButton();
  }

  async function openResult(result: PageResult) {
    currentSelectedPageId = result.page_id;
    pushCurrentState();
    message("");
    try {
      if (result.page_type === "knowledge") {
        showKnowledgePage(await api.openKnowledgePage(result.page_id));
      } else {
        showPage(await api.openSource(result.source_id));
      }
      if (result.match_location) highlightQuote(result.match_location.quote);
      syncBackButton();
    } catch (error) {
      report(error);
    }
  }

  async function restoreNavigation(state: NavigationState) {
    if (state.kind === "search") {
      await runSearch(state.request, false);
      currentSelectedPageId = state.selectedPageId;
      const selected = currentSelectedPageId
        ? main.querySelector<HTMLButtonElement>(
            `.search-result button[data-page-id="${CSS.escape(currentSelectedPageId)}"]`,
          )
        : null;
      if (selected) selected.focus({ preventScroll: true });
      main.scrollTop = state.scrollTop;
      window.scrollTo(0, state.windowScroll);
    } else if (state.kind === "source") {
      showPage(await api.openSource(state.sourceId));
      const article = main.querySelector<HTMLElement>("article");
      if (article) article.scrollTop = state.scrollTop;
      if (state.focusHref)
        Array.from(main.querySelectorAll<HTMLAnchorElement>("article a"))
          .find(
            (link) =>
              link.getAttribute("href") === state.focusHref &&
              link.textContent === state.focusText,
          )
          ?.focus({ preventScroll: true });
      window.scrollTo(0, state.windowScroll);
    } else {
      showKnowledgePage(await api.openKnowledgePage(state.pageId));
      const article = main.querySelector<HTMLElement>("article");
      if (article) article.scrollTop = state.scrollTop;
      if (state.focusHref)
        Array.from(main.querySelectorAll<HTMLAnchorElement>("article a"))
          .find(
            (link) =>
              link.getAttribute("href") === state.focusHref &&
              link.textContent === state.focusText,
          )
          ?.focus({ preventScroll: true });
      window.scrollTo(0, state.windowScroll);
    }
  }

  backButton.addEventListener("click", () => {
    const state = navigationHistory.pop();
    syncBackButton();
    if (state) void restoreNavigation(state).catch(report);
  });
  searchButton.addEventListener("click", () => {
    void runSearch(
      {
        query: "",
        tags: [],
        date_from: null,
        date_to: null,
        format: null,
        processing_status: null,
        offset: 0,
      },
      true,
    ).catch(report);
  });

  async function importPaths(paths: string[], method: AcquisitionMethod) {
    if (busy || disposed || paths.length === 0) return;
    busy = true;
    addButton.disabled = true;
    main.setAttribute("aria-busy", "true");
    for (const path of paths.slice(0, 100)) {
      message("Retaining source…");
      try {
        const imported = await api.importSource(path, method);
        showPage(imported);
        message(
          imported.info.extraction === "text_preserved"
            ? "Source added."
            : `${labels[imported.info.extraction]}. The original is retained.`,
        );
      } catch (error) {
        report(error);
      }
    }
    if (paths.length > 100)
      message(
        "Added the first 100 files. Drop the remaining files to continue.",
      );
    busy = false;
    if (!disposed) {
      addButton.disabled = false;
      main.removeAttribute("aria-busy");
    }
  }

  addButton.addEventListener("click", () => {
    void api
      .chooseFile()
      .then((path) => (path ? importPaths([path], "picker") : undefined))
      .catch(report);
  });

  function sourceButton(source: SourceSummary) {
    const button = element("button", source.title);
    button.addEventListener("click", () => {
      message("");
      void api.openSource(source.source_id).then(showPage).catch(report);
    });
    return button;
  }

  async function loadSources(offset: number) {
    const listing = await api.listSources(offset);
    if (disposed) return;
    const heading = element("h2", "Sources");
    sources.replaceChildren(heading);
    if (listing.sources.length === 0)
      sources.append(element("p", "No sources yet."));
    for (const source of listing.sources) sources.append(sourceButton(source));
    if (offset > 0) {
      const previous = element("button", "Previous sources");
      previous.addEventListener("click", () => {
        void loadSources(Math.max(0, offset - 50)).catch(report);
      });
      sources.append(previous);
    }
    if (listing.next_offset !== null) {
      const more = element("button", "More sources");
      more.addEventListener("click", () => {
        void loadSources(listing.next_offset!).catch(report);
      });
      sources.append(more);
    }
    sources.hidden = false;
    sourcesButton.setAttribute("aria-expanded", "true");
    sources.querySelector("button")?.focus();
  }
  sourcesButton.addEventListener("click", () => {
    if (!sources.hidden) {
      sources.hidden = true;
      sourcesButton.setAttribute("aria-expanded", "false");
    } else {
      void loadSources(0).catch(report);
    }
  });
  root.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      sources.hidden = true;
      sourcesButton.setAttribute("aria-expanded", "false");
      if (page) main.querySelector("article")?.focus();
      else sourcesButton.focus();
    }
  });

  const unlisten = await api.onDrop((paths) => {
    void importPaths(paths, "drop");
  });
  let lastSource: string | null = null;
  try {
    lastSource = localStorage.getItem(LAST_SOURCE);
  } catch {
    /* Optional navigation state. */
  }
  if (lastSource) {
    try {
      showPage(await api.openSource(lastSource));
    } catch (error) {
      report(error);
    }
  }
  const refreshTimer = window.setInterval(() => {
    const current = page;
    if (
      !current ||
      disposed ||
      current.info.semantic_state === "complete" ||
      current.info.semantic_state === "failed" ||
      current.info.semantic_state === "unavailable"
    )
      return;
    void api
      .openSource(current.info.source_id)
      .then((next) => {
        if (disposed || page?.info.source_id !== current.info.source_id) return;
        const detailsWereOpen = main.querySelector("details")?.open ?? false;
        const focusedArticle =
          document.activeElement === main.querySelector("article");
        const scrollTop = main.querySelector("article")?.scrollTop ?? 0;
        if (
          next.info.semantic_state === current.info.semantic_state &&
          next.info.semantic_error === current.info.semantic_error &&
          next.info.knowledge_pages.length ===
            current.info.knowledge_pages.length
        )
          return;
        showPage(next);
        const article = main.querySelector("article");
        if (article) article.scrollTop = scrollTop;
        const details = main.querySelector("details");
        if (details) details.open = detailsWereOpen;
        if (focusedArticle) article?.focus({ preventScroll: true });
      })
      .catch(report);
  }, 3000);
  return () => {
    disposed = true;
    window.clearInterval(refreshTimer);
    unlisten();
    root.replaceChildren();
  };
}
