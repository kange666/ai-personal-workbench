export interface TestFailureDiagnosis {
  category: string;
  stage: string;
  summary: string;
  possibleCauses: string[];
  troubleshootingSteps: string[];
  technicalDetails: string;
}

export function cleanTestOutput(value = "") {
  return value
    .replace(/[\u001b\u009b\ufffd]\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\r/g, "")
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "")
    .trim();
}

function timeoutLabel(error: string) {
  const milliseconds = error.match(/timeout(?:\s+of)?\s+(\d+)ms/i)?.[1];
  if (!milliseconds) return "测试在规定时间内没有完成";
  const value = Number(milliseconds);
  return value >= 60_000 && value % 60_000 === 0
    ? `测试超过 ${value / 60_000} 分钟仍未完成`
    : `测试超过 ${(value / 1000).toFixed(value % 1000 ? 1 : 0)} 秒仍未完成`;
}

export function diagnoseTestFailure(rawError: string, status = "failed", fallbackOutput = ""): TestFailureDiagnosis {
  const cleanedError = cleanTestOutput(rawError);
  const cleanedOutput = cleanTestOutput(fallbackOutput);
  const technicalDetails = [cleanedError, cleanedOutput && !cleanedError.includes(cleanedOutput) ? `原始命令输出摘要：\n${cleanedOutput.slice(0, 8000)}` : ""]
    .filter(Boolean)
    .join("\n\n") || "测试执行器没有返回可识别的错误详情。";
  const normalized = technicalDetails.toLowerCase();

  if (/timeout|timed out|超时|exceeded/.test(normalized)) {
    return {
      category: "执行超时",
      stage: /locator|expect|waiting for|element/.test(normalized) ? "页面交互或断言等待阶段" : "场景执行阶段",
      summary: `${timeoutLabel(technicalDetails)}，Playwright 一直没有等到当前操作或断言满足条件。`,
      possibleCauses: [
        "页面接口长时间没有返回，加载状态或遮罩层一直未结束。",
        "测试定位的按钮、表格或弹框没有出现，选择器可能与当前页面不一致。",
        "登录态、权限或 Token 失效，页面停留在登录页或无权限状态。",
        "测试环境响应较慢，或前置数据不满足当前业务场景。",
      ],
      troubleshootingSteps: [
        "先查看失败截图，确认超时时页面实际停留在哪一步。",
        "展开原始命令输出，搜索 locator、expect、page.goto 或最后一个接口地址。",
        "在浏览器 Network 中确认对应接口是否完成，并核对 HTTP 状态码和响应内容。",
        "检查测试中的元素定位方式、页面 loading 状态和该场景所需前置数据。",
      ],
      technicalDetails,
    };
  }
  if (/401|403|unauthorized|forbidden|token|登录|权限/.test(normalized)) {
    return {
      category: "登录或权限异常",
      stage: "身份验证或接口请求阶段",
      summary: "请求或页面访问没有通过身份验证，测试无法进入预期业务状态。",
      possibleCauses: ["环境变量中的 Token 已过期或没有传入测试进程。", "当前账号缺少该菜单或接口权限。", "接口使用的请求头名称与测试配置不一致。"],
      troubleshootingSteps: ["确认工作台读取的是当前 Windows 用户环境变量。", "用同一账号手工进入页面并执行相同操作。", "检查失败请求的请求头、HTTP 状态码和响应消息。"],
      technicalDetails,
    };
  }
  if (/404|not found/.test(normalized)) {
    return {
      category: "地址或资源不存在",
      stage: "页面导航或接口请求阶段",
      summary: "页面、接口或测试依赖的资源返回了 404，当前访问地址没有对应内容。",
      possibleCauses: ["页面路由或接口地址拼接错误。", "开发代理没有把请求转发到正确服务。", "后端版本与当前前端接口路径不一致。"],
      troubleshootingSteps: ["从原始输出中确认完整请求地址。", "核对项目代理配置和后端服务地址。", "直接访问接口并确认该环境是否部署了对应版本。"],
      technicalDetails,
    };
  }
  if (/5\d\d|internal server error|bad gateway|service unavailable/.test(normalized)) {
    return {
      category: "服务端异常",
      stage: "接口请求与响应阶段",
      summary: "接口返回了服务端错误，页面无法获得完成当前场景所需的数据。",
      possibleCauses: ["后端服务内部报错或依赖服务不可用。", "测试参数或前置数据触发了服务端异常。", "当前环境部署状态不完整。"],
      troubleshootingSteps: ["确认失败接口的状态码、请求参数和响应正文。", "查看对应时间点的后端日志。", "使用相同参数单独重放接口以缩小问题范围。"],
      technicalDetails,
    };
  }
  if (/locator|strict mode|waiting for|not visible|not attached|element/.test(normalized)) {
    return {
      category: "页面元素未找到",
      stage: "页面元素定位与交互阶段",
      summary: "Playwright 没有找到或无法操作测试所需的页面元素。",
      possibleCauses: ["页面结构或按钮文字已经变化，原定位器失效。", "元素位于未展开的弹框、抽屉或 Tab 中。", "接口未返回导致目标区域没有渲染。"],
      troubleshootingSteps: ["结合截图确认目标控件是否实际显示。", "根据调用栈中的文件和行号定位具体 locator。", "检查元素是否需要先切换 Tab、展开弹框或等待接口完成。"],
      technicalDetails,
    };
  }
  if (/expect|expected|received|assert|断言/.test(normalized)) {
    return {
      category: "结果与预期不一致",
      stage: "结果断言阶段",
      summary: "页面或接口已经返回结果，但实际值没有满足测试断言。",
      possibleCauses: ["业务逻辑发生变化，测试预期尚未同步。", "接口数据或页面状态确实不正确。", "测试环境的基础数据与用例假设不同。"],
      troubleshootingSteps: ["对比错误中的 Expected 和 Received。", "核对接口响应、页面展示和业务规则三者是否一致。", "确认是产品缺陷还是用例预期需要更新。"],
      technicalDetails,
    };
  }
  if (/net::err|connection refused|dns|page\.goto|navigation/.test(normalized)) {
    return {
      category: "页面或网络不可达",
      stage: "页面导航阶段",
      summary: "浏览器没有成功打开目标页面，后续业务步骤无法执行。",
      possibleCauses: ["前端项目尚未启动或访问端口不正确。", "代理、DNS 或网络连接异常。", "页面跳转到了错误地址。"],
      troubleshootingSteps: ["确认项目运行状态和测试使用的完整 URL。", "在浏览器中手工打开相同地址。", "检查开发服务器、代理和端口占用情况。"],
      technicalDetails,
    };
  }

  return {
    category: status === "blocked" ? "环境条件不满足" : "测试执行异常",
    stage: status === "blocked" ? "执行前环境检查阶段" : "测试执行阶段",
    summary: status === "blocked" ? "测试所需的运行环境或前置条件没有通过检查。" : "测试执行器报告了异常，但当前错误不属于已识别的常见类型。",
    possibleCauses: ["页面事件、接口响应或测试数据没有达到用例预期。", "测试脚本与当前业务页面结构不一致。", "运行环境、依赖或前置条件存在异常。"],
    troubleshootingSteps: ["先阅读下方完整技术详情和调用栈。", "结合失败截图定位最后成功完成的页面步骤。", "展开原始命令输出，优先处理最早出现的错误。"],
    technicalDetails,
  };
}
