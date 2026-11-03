import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const skillDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(skillDir, "../../..");

export default defineConfig({
  root: repoRoot,
  server: { port: 1420, strictPort: true },
  test: {
    environment: "jsdom",
    include: [path.join(skillDir, "journeys/**/*.test.ts")],
    restoreMocks: true,
    testTimeout: 120_000,
    hookTimeout: 180_000,
  },
});
