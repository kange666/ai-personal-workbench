import { describe, expect, it } from "vitest";
import { latestTestsByProjectAndMode, notificationsChanged } from "./workStatus";
import type { TestRun, WorkbenchNotification } from "../services/backend";
describe("右栏状态", () => {
  it("不同项目和测试类型不互相覆盖，只保留各组最近一次", () => {
    const run=(id:string,projectPath:string,mode:string,startedAt:string) => ({id,projectPath,menuId:"same-menu",mode,startedAt}) as TestRun;
    const runs=[run("a1","C:\\alpha","mock","2026-01-01"),run("a2","c:/ALPHA/","mock","2026-01-02"),run("b1","C:/beta","mock","2026-01-01"),run("a-real","C:/alpha","real","2026-01-01")];
    expect(latestTestsByProjectAndMode(runs).map(item => item.id).sort()).toEqual(["a2","a-real","b1"].sort());
  });
  it("相同通知不广播，已读/内容变化才广播", () => {
    const notices=[{id:"n1",isRead:false}] as WorkbenchNotification[];
    expect(notificationsChanged(notices,structuredClone(notices))).toBe(false);
    expect(notificationsChanged(notices,[{...notices[0],isRead:true}])).toBe(true);
  });
});
