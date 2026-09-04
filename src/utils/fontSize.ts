export const fontSizeOptions = [
  { value: "small", label: "小", offset: -2 },
  { value: "medium", label: "中", offset: 0 },
  { value: "large", label: "大", offset: 2 },
  { value: "extra-large", label: "超大", offset: 4 },
] as const;

export type FontSize = typeof fontSizeOptions[number]["value"];
const storageKey = "workbench-font-size-v1";

export function loadFontSize(): FontSize {
  try {
    const saved = localStorage.getItem(storageKey);
    return fontSizeOptions.find((option) => option.value === saved)?.value ?? "medium";
  } catch {
    return "medium";
  }
}

export function applyFontSize(value: FontSize): void {
  const option = fontSizeOptions.find((item) => item.value === value) ?? fontSizeOptions[1];
  // 只改变字号偏移，不缩放窗口、卡片、间距和图标；各处最小字号由样式统一限制。
  document.documentElement.style.setProperty("--app-font-offset", `${option.offset}px`);
  document.documentElement.dataset.fontSize = option.value;
}

export function saveFontSize(value: FontSize): void {
  // 先保存再应用：存储失败时保持原选择，让设置页明确提示失败。
  localStorage.setItem(storageKey, value);
  applyFontSize(value);
}
