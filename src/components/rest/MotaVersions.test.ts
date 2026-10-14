import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { expect, it, vi } from "vitest";
import MotaVersions from "./MotaVersions.vue";
const reset21 = vi.fn(), reset50 = vi.fn();
const game = (name: string, restart: () => void) => defineComponent({ props: ["paused"], setup(_, { expose }) { expose({ restart }); }, template: `<div>${name} {{ paused }}</div>` });
it("先选版本、切换时卸载当前版本、暂停和重开只作用于所选游戏", async () => {
  reset21.mockClear(); reset50.mockClear();
  const wrapper = mount(MotaVersions, { props: { paused: false }, global: { stubs: { MotaGame: game("21 层", reset21), Mota50Game: game("50 层", reset50) } } });
  try {
    expect(wrapper.findComponent({ name: "MotaGame" }).exists()).toBe(false);
    await wrapper.get(".mota-version").trigger("click");
    expect(wrapper.text()).toContain("21 层 false");
    (wrapper.vm as unknown as { restart(): void }).restart();
    expect(reset21).toHaveBeenCalledOnce(); expect(reset50).not.toHaveBeenCalled();
    await wrapper.get(".mota-version-bar button").trigger("click");
    await wrapper.findAll(".mota-version")[1]!.trigger("click");
    await wrapper.setProps({ paused: true }); await flushPromises();
    expect(wrapper.text()).toContain("50 层 true"); expect(wrapper.text()).not.toContain("21 层 false");
    (wrapper.vm as unknown as { restart(): void }).restart(); expect(reset50).toHaveBeenCalledOnce();
  } finally { wrapper.unmount(); }
});
