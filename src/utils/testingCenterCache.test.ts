import { beforeEach, describe, expect, it } from "vitest";
import { readTestingMenuCache, writeTestingMenuCache } from "./testingCenterCache";

describe("测试中心菜单会话缓存", () => {
  beforeEach(() => window.sessionStorage.clear());

  it("同一项目再次进入时可以立即读取菜单", () => {
    const menus = [{ id: "post", name: "岗位管理" }];
    writeTestingMenuCache(window.sessionStorage, "F:/TB-project/client", menus, 1_000);

    expect(readTestingMenuCache(window.sessionStorage, "f:\\TB-project\\client\\", 2_000)).toEqual(menus);
  });

  it("过期缓存和损坏缓存不会被当成真实菜单", () => {
    writeTestingMenuCache(window.sessionStorage, "F:/project", [{ id: "old" }], 1_000);
    expect(readTestingMenuCache(window.sessionStorage, "F:/project", 31 * 60 * 1_000)).toEqual([]);

    window.sessionStorage.setItem("ai-workbench.testing.menu-cache.v1", "{invalid");
    expect(readTestingMenuCache(window.sessionStorage, "F:/project", 2_000)).toEqual([]);
  });
});
