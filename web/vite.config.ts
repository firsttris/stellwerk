import { paraglideVitePlugin } from "@inlang/paraglide-js";
import tailwindcss from "@tailwindcss/vite";
import { redact } from "@tanstack/redact/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// `vite` talks to a Zentrale on :8080; `vite --mode demo` uses the fixtures instead.
export default defineConfig(({ mode }) => ({
  plugins: [
    paraglideVitePlugin({
      project: "./project.inlang",
      outdir: "./src/paraglide",
      outputStructure: "message-modules",
      strategy: ["localStorage", "preferredLanguage", "baseLocale"],
    }),
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    react(),
    // Redact replaces the React runtime (smaller bundle, the code keeps importing from `react`);
    // rendering is synchronous, and nothing here relies on concurrent scheduling. Not in Vitest:
    // Testing Library is CommonJS and requires the real react-dom, which would render the app's
    // Redact hooks. The E2E tests run the build with Redact.
    ...(mode === "test" ? [] : [redact()]),
    tailwindcss(),
  ],
  server: {
    proxy: {
      "/api": { target: "http://127.0.0.1:8080", ws: true },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
}));
