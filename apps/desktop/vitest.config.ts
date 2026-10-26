// SPDX-License-Identifier: MIT OR Apache-2.0
// Component and unit tests (Phase 5): jsdom, the fake Tauri bridge from src/test/setup.ts.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify("0.0.0-test") },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // Dates render in en-GB; keep the clock zone fixed so snapshots match everywhere.
    env: { TZ: "UTC" },
  },
});
