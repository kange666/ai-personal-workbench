import { describe, expect, it } from "vitest";
import { cleanTestOutput, diagnoseTestFailure } from "./testFailureDiagnosis";

describe("测试失败诊断", () => {
  it("把 Playwright 超时转换为可排查的中文说明", () => {
    const diagnosis = diagnoseTestFailure("\u001b[31mTest timeout of 60000ms exceeded.\u001b[39m\n at locator.click (related.spec.js:42:8)");

    expect(diagnosis.category).toBe("执行超时");
    expect(diagnosis.summary).toContain("1 分钟");
    expect(diagnosis.possibleCauses).toContain("登录态、权限或 Token 失效，页面停留在登录页或无权限状态。");
    expect(diagnosis.troubleshootingSteps[0]).toContain("失败截图");
    expect(diagnosis.technicalDetails).toContain("related.spec.js:42:8");
    expect(diagnosis.technicalDetails).not.toContain("[31m");
  });

  it("能区分权限、404 和元素定位问题", () => {
    expect(diagnoseTestFailure("HTTP 403 Forbidden").category).toBe("登录或权限异常");
    expect(diagnoseTestFailure("GET /related/list returned 404 Not Found").category).toBe("地址或资源不存在");
    expect(diagnoseTestFailure("locator('button').click: element is not visible").category).toBe("页面元素未找到");
  });

  it("会清理损坏的终端颜色控制码", () => {
    expect(cleanTestOutput("�[31m失败�[39m\r\n详情")).toBe("失败\n详情");
  });
});
