import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import { confirmAction, resetConfirmQueueForTests } from "../utils/confirm";

afterEach(() => {
  resetConfirmQueueForTests();
  document.body.innerHTML = "";
});

describe("ConfirmDialog", () => {
  it("uses the workbench panel instead of the browser confirm dialog", async () => {
    const wrapper = mount(ConfirmDialog, {
      attachTo: document.body,
      global: { stubs: { Teleport: true, Transition: false } },
    });

    const result = confirmAction({
      title: "确认发布",
      message: "项目：demo\n分支：main",
      confirmText: "开始发布",
      tone: "warning",
    });
    await nextTick();

    expect(wrapper.find('[role="alertdialog"]').exists()).toBe(true);
    expect(wrapper.find("h2").text()).toBe("确认发布");
    expect(wrapper.text()).toContain("项目：demo");
    expect(wrapper.find(".warning").exists()).toBe(true);
    const buttons = wrapper.findAll("footer button");
    expect(buttons.map((button) => button.text())).toEqual(["取消", "开始发布"]);
    await buttons[1].trigger("click");
    await expect(result).resolves.toBe(true);
    wrapper.unmount();
  });
});
