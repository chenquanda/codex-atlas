import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/.tmp/**", "**/.cache/**", "**/dist/**"]
    }
  },
  build: {
    emptyOutDir: false
  },
  test: {
    exclude: [...configDefaults.exclude, "src/smoke/**"]
  }
});
