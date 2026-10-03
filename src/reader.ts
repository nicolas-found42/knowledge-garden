import DOMPurify from "dompurify";
import { marked } from "marked";
import type {
  AcquisitionMethod,
  GardenApi,
  KnowledgePage,
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
  const sourcesButton = element("button", "Sources");
  sourcesButton.setAttribute("aria-expanded", "false");
  const addButton = element("button", "Add source");
  addButton.className = "primary";
  toolbar.append(brand, sourcesButton, addButton);
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
  let busy = false;
  let disposed = false;

  function message(text: string) {
    if (!disposed) notice.textContent = text;
  }
  function report(error: unknown) {
    message(error instanceof Error ? error.message : String(error));
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
        void api
          .openKnowledgePage(knowledgeMatch[1])
          .then(showKnowledgePage)
          .catch(report);
      } else if (sourceMatch) {
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
    const article = showMarkdown(next.markdown, next.title, next.source_id);
    if (!article) return;
    const returnLink = element("a", "Return to source page");
    returnLink.href = `../sources/${next.source_id.slice("source-".length)}/index.md`;
    article.prepend(returnLink);
  }

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
