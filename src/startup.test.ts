// @vitest-environment jsdom
import { expect, it } from "vitest";
import { waitForRecovery } from "./startup";

it("shows recovery and continues automatically after it completes", async () => {
  const root = document.createElement("div");
  let phase = "recovering";
  let visible = false;
  await waitForRecovery(
    root,
    async () => ({ phase, last_issue: null }),
    async () => {
      visible = root.textContent?.includes("Recovering your library") ?? false;
      phase = "waiting";
    },
  );
  expect(visible).toBe(true);
});

it("shows a recovery issue while waiting for automatic recovery", async () => {
  const root = document.createElement("div");
  let phase = "recovering";
  let visible = false;
  await waitForRecovery(
    root,
    async () => ({ phase, last_issue: "Disk is full" }),
    async () => {
      visible = root.textContent?.includes("Disk is full") ?? false;
      phase = "waiting";
    },
  );
  expect(visible).toBe(true);
});
