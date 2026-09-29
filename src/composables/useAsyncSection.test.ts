import { describe, expect, it, vi } from "vitest";
import { useAsyncSection } from "./useAsyncSection";
describe("独立卡片读取", () => {
  it("失败保留数据和上次成功时间，重试只作用于本卡", async () => {
    let data="";
    const read=vi.fn(async () => { data="成功数据"; });
    const section=useAsyncSection(read);
    await section.load(); const success=section.state.updatedAt;
    read.mockRejectedValueOnce(new Error("busy")); await section.load();
    expect(data).toBe("成功数据"); expect(section.state.updatedAt).toBe(success); expect(section.state.error).toContain("busy");
    await section.load(); expect(section.state.error).toBe("");
  });
  it("读取中多次来源更新合并为一次后续重算", async () => {
    let finish!: () => void;
    const read=vi.fn<(refresh:boolean)=>Promise<void>>().mockImplementationOnce(() => new Promise<void>(resolve => {finish=resolve;})).mockResolvedValue(undefined);
    const section=useAsyncSection(read); const first=section.load();
    void section.load(true); void section.load(true); expect(read).toHaveBeenCalledTimes(1);
    finish(); await first;
    expect(read).toHaveBeenCalledTimes(2); expect(read).toHaveBeenLastCalledWith(true);
  });
});
