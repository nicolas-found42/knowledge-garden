import type { GardenApi, GapInspection } from "./api";
const names = {
  source_unavailable: "Unavailable source",
  extraction_incomplete: "Incomplete extraction",
  update_pending: "Pending update",
  update_failed: "Failed or uncertain update",
  identity_ambiguous: "Ambiguous identity",
  insufficient_support: "Insufficient or qualified support",
};
/** Collapsed by default; explanation never asks the owner to approve knowledge. */
export function gapInspector(
  api: Pick<GardenApi, "inspectGaps" | "openOriginal" | "openOriginalVersion">,
  pageId: string,
  report: (error: unknown) => void,
) {
  const panel = document.createElement("details");
  panel.className = "gap-inspector";
  const summary = document.createElement("summary");
  summary.textContent = "Inspect knowledge gaps";
  const content = document.createElement("div");
  const refresh = document.createElement("button");
  refresh.textContent = "Refresh gap inspection";
  const status = document.createElement("p");
  status.setAttribute("role", "status");
  let sequence = 0;
  const draw = (result: GapInspection) => {
    content.replaceChildren();
    for (const gap of result.gaps) {
      const section = document.createElement("section");
      const heading = document.createElement("h3");
      heading.textContent = names[gap.category];
      section.append(heading);
      for (const text of [
        `Affected scope: ${gap.scope}`,
        gap.detail,
        gap.recovery,
      ]) {
        const p = document.createElement("p");
        p.textContent = text;
        section.append(p);
      }
      if (gap.evidence) {
        const evidence = document.createElement("blockquote");
        evidence.textContent = gap.evidence;
        section.append(evidence);
      }
      if (gap.source_id && gap.original_available) {
        const original = document.createElement("button");
        original.textContent = "Open gap source original";
        original.addEventListener("click", () => {
          const operation = gap.original_version
            ? api.openOriginalVersion(
                gap.source_id!,
                gap.original_version[0],
                gap.original_version[1],
              )
            : api.openOriginal(gap.source_id!);
          void operation.catch(report);
        });
        section.append(original);
      }
      content.append(section);
    }
    status.textContent = result.gaps.length
      ? `${result.gaps.length} gap explanations${result.truncated ? "; bounded inspection shortened" : ""}.`
      : "No gaps reported for this page's inspected scope.";
  };
  const load = async () => {
    const token = ++sequence;
    status.textContent = "Inspecting current gaps…";
    try {
      const result = await api.inspectGaps!(pageId);
      if (panel.isConnected && token === sequence) draw(result);
    } catch (error) {
      if (panel.isConnected && token === sequence) {
        content.replaceChildren();
        status.textContent = `Gap inspection unavailable: ${String(error)}`;
        report(error);
      }
    }
  };
  refresh.addEventListener("click", () => void load());
  panel.addEventListener("toggle", () => {
    if (panel.open) void load();
  });
  panel.append(summary, refresh, status, content);
  return panel;
}
