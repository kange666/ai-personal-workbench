import type { Plugin } from "postcss";

/** 编译时统一处理全局和 Vue scoped 字号，避免逐页覆盖或运行时扫描 DOM。 */
export function fontSizePlugin(): Plugin {
  const adjustable = (size: string) => `max(10px, calc(${size} + var(--app-font-offset, 0px)))`;
  return {
    postcssPlugin: "workbench-font-size",
    Declaration(declaration) {
      // 只处理原有的像素字号；继承和图标变量保持原样，避免重复累加。
      if (declaration.prop === "font-size" && /^\d+(?:\.\d+)?px$/i.test(declaration.value)) {
        declaration.value = adjustable(declaration.value);
      } else if (declaration.prop === "font") {
        // shorthand 的第一个 px token 是字号；不匹配行高或引号内的字体名称。
        const match = declaration.value.match(/^((?:(?:normal|italic|oblique|small-caps|bold|bolder|lighter|[1-9]00)\s+)*)(\d+(?:\.\d+)?px)(?=[\s/])/i);
        if (match) declaration.value = declaration.value.replace(match[0], match[1] + adjustable(match[2]));
      }
    },
  };
}
