import { defineConfig } from "vitest/config";

// The end-to-end tests drive the real app (harness.ts), one at a time: the
// app runs as a single instance.
export default defineConfig({
  test: {
    include: ["e2e/**/*.e2e.ts"],
    setupFiles: ["e2e/setup.ts"],
    fileParallelism: false,
    testTimeout: 180_000,
    hookTimeout: 60_000,
  },
});
