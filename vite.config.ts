import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";
import { fontSizePlugin } from "./scripts/font-size-plugin";

export default defineConfig({
  plugins: [vue()],
  // 所有页面（含懒加载页面和 scoped 样式）使用同一字号偏移，保留原布局尺寸。
  css: { postcss: { plugins: [fontSizePlugin()] } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  test: {
    environment: "jsdom",
    clearMocks: true,
    pool: "threads",
    fileParallelism: false,
  },
});
