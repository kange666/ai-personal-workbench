import { describe, expect, it } from "vitest";
import { compactDetailTitle } from "./detailTitle";

describe("compactDetailTitle", () => {
  it("uses the readable conversation label instead of ids and the full request", () => {
    const title = "/goal [流程审批详情渲染方案](chatgpt-conversation://6a5438be-d63c-83ee-8252-d08df58a9481) 以此方案为基础，修改现在的我的任务页面，新增详情抽屉";
    expect(compactDetailTitle(title, "client")).toBe("client：流程审批详情渲染方案");
  });

  it("keeps an existing project and summary title", () => {
    expect(compactDetailTitle("个人工作台：窗口按钮优化")).toBe("个人工作台：窗口按钮优化");
  });

  it("removes command prefixes and keeps a short project summary", () => {
    expect(compactDetailTitle("/goal （Uniapp Jenkins 自动部署）", "APP")).toBe(
      "APP：Uniapp Jenkins 自动部署",
    );
  });
});
