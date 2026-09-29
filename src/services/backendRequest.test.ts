import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { backendLoadingEvent, backgroundRequest, beginBackendPageScope, invoke } from "./backendRequest";
vi.mock("@tauri-apps/api/core", () => ({ invoke:vi.fn() }));

describe("页面请求与后台隔离", () => {
  beforeEach(() => { vi.clearAllMocks(); beginBackendPageScope(); });
  it("后台请求不改变页面计数，前台失败也会正确释放", async () => {
    const counts:number[]=[];
    const listener=(event:Event) => counts.push((event as CustomEvent).detail.active);
    window.addEventListener(backendLoadingEvent,listener);
    let end!: () => void;
    vi.mocked(tauriInvoke).mockReturnValueOnce(new Promise<void>(resolve => { end=resolve; })).mockRejectedValueOnce(new Error("failed"));
    const background=backgroundRequest(() => invoke("background"));
    expect(counts).toEqual([]);
    await expect(invoke("foreground")).rejects.toThrow("failed");
    expect(counts).toEqual([1,0]);
    end(); await background; expect(counts).toEqual([1,0]);
    window.removeEventListener(backendLoadingEvent,listener);
  });
  it("上一页慢请求不阻塞下一页，也不能提前释放下一页请求", async () => {
    const counts:number[]=[];
    const listener=(event:Event) => counts.push((event as CustomEvent).detail.active);
    window.addEventListener(backendLoadingEvent,listener);
    let first!: () => void; let second!: () => void;
    vi.mocked(tauriInvoke).mockReturnValueOnce(new Promise<void>(resolve => { first=resolve; })).mockReturnValueOnce(new Promise<void>(resolve => { second=resolve; }));
    const old=invoke("old"); beginBackendPageScope(); const current=invoke("current");
    first(); await old; expect(counts.at(-1)).toBe(1);
    second(); await current; expect(counts.at(-1)).toBe(0);
    window.removeEventListener(backendLoadingEvent,listener);
  });
});
