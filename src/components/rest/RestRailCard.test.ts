import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import RestRailCard from "./RestRailCard.vue";
import { restGames } from "../../utils/rest/catalog";

vi.mock("./RestGameDialog.vue", () => ({ default: {
  props: ["game"], emits: ["close"],
  template: '<div role="dialog" :data-game="game"><button @click="$emit(\'close\')">关闭</button></div>',
} }));

it("无文字图标入口能逐个打开对应游戏，关闭后可以重新选择", async () => {
  const wrapper = mount(RestRailCard);
  expect(wrapper.get(".rail-rest-card").text()).toBe("");
  expect(wrapper.findAll(".rail-rest-game")).toHaveLength(6);
  for (const game of restGames) {
    await wrapper.get(`[aria-label="打开${game.name}"]`).trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="dialog"]').attributes("data-game")).toBe(game.id);
    await wrapper.get('[role="dialog"] button').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
  }
  wrapper.unmount();
});
