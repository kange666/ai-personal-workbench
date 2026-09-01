import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  loadHiddenNavigationPaths,
  loadNavigationOrder,
  navigationOrderChangedEvent,
  saveHiddenNavigationPaths,
  workbenchNavigationItems,
} from "./navigation";

describe("navigation settings", () => {
  beforeEach(() => localStorage.clear());

  it("旧用户没有隐藏配置时默认显示全部菜单", () => {
    expect(loadHiddenNavigationPaths()).toEqual([]);
    expect(loadNavigationOrder()).toHaveLength(workbenchNavigationItems.length);
  });

  it("只保存有效且不重复的隐藏菜单路径，并通知应用外壳刷新", () => {
    const listener = vi.fn();
    window.addEventListener(navigationOrderChangedEvent, listener);

    expect(saveHiddenNavigationPaths(["/calendar", "/calendar", "/not-exists"])).toEqual(["/calendar"]);
    expect(loadHiddenNavigationPaths()).toEqual(["/calendar"]);
    expect(listener).toHaveBeenCalledOnce();

    window.removeEventListener(navigationOrderChangedEvent, listener);
  });

  it("隐藏配置损坏时安全恢复为全部显示", () => {
    localStorage.setItem("workbench-navigation-hidden-v1", "not-json");
    expect(loadHiddenNavigationPaths()).toEqual([]);
  });
});
