import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TaskEditor from "./TaskEditor.vue";
const actions=vi.hoisted(() => ({ addTask:vi.fn(),updateTask:vi.fn(),removeTask:vi.fn() }));
vi.mock("../stores/workbench", () => ({useWorkbenchStore:() => actions}));
beforeEach(() => { vi.resetAllMocks(); });

describe("任务编辑保存反馈", () => {
  it("等待成功再关闭，失败保留输入并允许重试", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/",component:{template:"<div/>"}}]});
    await router.push("/");
    const wrapper=mount(TaskEditor,{props:{open:true},global:{plugins:[router]}});
    const title=wrapper.get('input[placeholder="输入任务名称"]'); await title.setValue("尚未保存的任务");
    let fail!: (cause: Error) => void;
    actions.addTask.mockReturnValueOnce(new Promise((_resolve,reject) => {fail=reject;}));
    await wrapper.get("footer .button.primary").trigger("click");
    expect(wrapper.emitted("close")).toBeUndefined(); expect(wrapper.get("footer .button.primary").attributes("disabled")).toBeDefined();
    fail(new Error("busy")); await flushPromises();
    expect(wrapper.emitted("close")).toBeUndefined();
    expect((title.element as HTMLInputElement).value).toBe("尚未保存的任务");
    expect(wrapper.get('[role="alert"]').text()).toContain("输入已保留");
    actions.addTask.mockResolvedValueOnce(undefined);
    await wrapper.get("footer .button.primary").trigger("click"); await flushPromises();
    expect(wrapper.emitted("close")).toHaveLength(1); expect(actions.addTask).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });
});
