import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./src/smoke",
  timeout: 30_000,
  expect: {
    timeout: 5_000
  },
  fullyParallel: false,
  reporter: [["list"]],
  outputDir: ".tmp/playwright-results",
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:1420",
    trace: "retain-on-failure",
    viewport: {
      width: 1280,
      height: 900
    }
  },
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium"
      }
    }
  ]
});
