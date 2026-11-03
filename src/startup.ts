type RecoveryStatus = {
  phase: string;
  last_issue: string | null;
  recovery_detail: string | null;
};

function readRecoveryStatus(value: unknown): RecoveryStatus {
  if (
    typeof value !== "object" ||
    value === null ||
    !("phase" in value) ||
    typeof value.phase !== "string" ||
    !("last_issue" in value) ||
    (typeof value.last_issue !== "string" && value.last_issue !== null)
  ) {
    throw new Error("The library recovery status is unavailable.");
  }
  const detail = "recovery_detail" in value ? value.recovery_detail : null;
  if (typeof detail !== "string" && detail !== null) {
    throw new Error("The library recovery status is unavailable.");
  }
  return {
    phase: value.phase,
    last_issue: value.last_issue,
    recovery_detail: detail,
  };
}

export async function waitForRecovery(
  root: HTMLElement,
  readStatus: () => Promise<unknown>,
  wait: () => Promise<void> = () =>
    new Promise((resolve) => window.setTimeout(resolve, 1000)),
): Promise<void> {
  for (;;) {
    const status = readRecoveryStatus(await readStatus());
    if (status.phase !== "recovering") return;
    const view = document.createElement("main");
    view.setAttribute("role", "status");
    const title = document.createElement("h1");
    title.textContent = "Recovering your library";
    const detail = document.createElement("p");
    detail.textContent = status.last_issue
      ? `Recovery is waiting: ${status.last_issue}`
      : (status.recovery_detail ??
        "Checking and preserving your collection. This may take a while.");
    view.append(title, detail);
    root.replaceChildren(view);
    await wait();
  }
}
