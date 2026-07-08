import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import devServer from "@hono/vite-dev-server";
import cloudflareAdapter from "@hono/vite-dev-server/cloudflare";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    devServer({
      entry: "src/server.ts",
      adapter: cloudflareAdapter,
      exclude: [
        /^\/(?!api|auth)/, // Let Hono handle /api/* and /auth/* routes, and exclude the rest for Vite SPA serving
        /^\/assets\//,
        /^\/@vite\//,
        /^\/node_modules\//,
        /^\/src\//
      ]
    })
  ],
  server: {
    port: 8080, // match the original port of the app
    watch: {
      ignored: ["**/.wrangler/**"]
    }
  },
  resolve: {
    alias: {
      "~": "/src"
    }
  },
  build: {
    outDir: "dist",
    emptyOutDir: true
  }
});
