import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true
  },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    globals: true,
    // Example provider scripts use Node's built-in test runner, not Vitest.
    exclude: ["**/node_modules/**", "examples/remote-providers/**/*.test.cjs"]
  }
});
