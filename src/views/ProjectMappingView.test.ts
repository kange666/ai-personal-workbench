import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as backend from "../services/backend";
import ProjectMappingView from "./ProjectMappingView.vue";

vi.mock("../services/backend");
let wrapper: VueWrapper;
const profile: backend.ProjectProfile = { id:"pc", displayName:"PC", repositoryPath:"F:/new/client", tapdWorkspaceId:"", aliases:[], category:"", createdAt:"now", updatedAt:"now" };
const counts = { conversations:0, testRuns:0, workSessions:0 };
const result: backend.ProjectMappingPreview = { before:{...counts,conversations:1}, after:{...counts,conversations:2}, added:{...counts,conversations:1}, removed:counts, unmatched:counts, conflicts:[], blockingConflicts:[] };

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
  vi.mocked(backend.listProjectProfiles).mockResolvedValue([profile]);
  vi.mocked(backend.previewProjectProfile).mockResolvedValue(result);
  vi.mocked(backend.saveProjectProfile).mockResolvedValue(profile);
});
afterEach(() => { wrapper?.unmount(); vi.useRealTimers(); });
async function render() {
  wrapper = mount(ProjectMappingView, { global:{ stubs:{ RouterLink:true } } });
  await flushPromises();
  await vi.advanceTimersByTimeAsync(350);
  await flushPromises();
}

describe("项目身份影响预览", () => {
  it("编辑只预览，保存才写入，并展示历史关联数量", async () => {
    await render();
    expect(wrapper.get('[aria-label="项目映射影响预览"]').text()).toContain("1 → 2");
    await wrapper.get("textarea").setValue("legacy\nF:/old/client");
    expect(wrapper.get('button[type="submit"], .mapping-editor>footer button').attributes("disabled")).toBeDefined();
    await vi.advanceTimersByTimeAsync(350);
    await flushPromises();
    expect(backend.previewProjectProfile).toHaveBeenLastCalledWith(expect.objectContaining({ aliases:["legacy","F:/old/client"] }));
    expect(backend.saveProjectProfile).not.toHaveBeenCalled();
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(backend.saveProjectProfile).toHaveBeenCalledOnce();
  });

  it("冲突别名阻止保存，失败保留输入", async () => {
    vi.mocked(backend.previewProjectProfile).mockResolvedValue({ ...result, conflicts:["APP 被其他项目使用"], blockingConflicts:["APP 被其他项目使用"] });
    await render();
    await wrapper.get("form").trigger("submit");
    expect(backend.saveProjectProfile).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("调整新增的冲突");
    await wrapper.get("textarea").setValue("unique-alias");
    vi.mocked(backend.previewProjectProfile).mockRejectedValue(new Error("读取失败"));
    await vi.advanceTimersByTimeAsync(350);
    await flushPromises();
    expect(wrapper.get("textarea").element).toHaveProperty("value", "unique-alias");
    expect(wrapper.text()).toContain("影响预览读取失败");
    vi.mocked(backend.previewProjectProfile).mockResolvedValue(result);
    await wrapper.findAll("button").find(button => button.text() === "重试预览")!.trigger("click");
    await flushPromises();
    expect(wrapper.get("textarea").element).toHaveProperty("value", "unique-alias");
    expect(wrapper.get('.mapping-editor>footer button').attributes("disabled")).toBeUndefined();
  });

  it("迟到的旧预览不能覆盖新输入的结果", async () => {
    await render();
    let resolveOld!: (value: backend.ProjectMappingPreview) => void;
    vi.mocked(backend.previewProjectProfile).mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
    await wrapper.get("textarea").setValue("old-alias");
    await vi.advanceTimersByTimeAsync(350);
    await wrapper.get("textarea").setValue("new-alias");
    await vi.advanceTimersByTimeAsync(350);
    await flushPromises();
    resolveOld({ ...result, blockingConflicts:["旧冲突"] });
    await flushPromises();
    expect(wrapper.text()).not.toContain("旧冲突");
    expect(wrapper.get('.mapping-editor>footer button').attributes("disabled")).toBeUndefined();
  });
});
