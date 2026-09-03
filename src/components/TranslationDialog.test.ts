import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(() => true),
  translateText: vi.fn(),
  translateError: vi.fn(),
}));

vi.mock("../services/backend", () => backend);

import TranslationDialog from "./TranslationDialog.vue";

const errorResult = {
  meaning: "TypeError：无法读取 undefined 的 name 属性。",
  possibleCauses: ["对象可能尚未初始化，需结合调用位置确认。"],
  solutions: ["检查对象赋值和异步请求是否完成。", "按业务要求处理空值，再使用相同输入验证。"],
};

function mountDialog() {
  return mount(TranslationDialog, {
    props: { open: true },
    global: {
      stubs: {
        RouterLink: { template: "<a><slot /></a>" },
      },
    },
  });
}

function action(wrapper: ReturnType<typeof mountDialog>, label: string) {
  const button = wrapper.findAll("button").find(item => item.text() === label);
  if (!button) throw new Error(`没有找到按钮：${label}`);
  return button;
}

describe("TranslationDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    backend.isTauriRuntime.mockReturnValue(true);
    backend.translateText.mockResolvedValue([
      { label: "自然表达", text: "Hello, world." },
      { label: "简洁表达", text: "Hello world." },
    ]);
    backend.translateError.mockResolvedValue(errorResult);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  });

  it("自动识别中英文，并把中英混合文本按中译英处理", async () => {
    const wrapper = mountDialog();
    const input = wrapper.get<HTMLTextAreaElement>(".translation-source textarea");

    await input.setValue("你好，Hello");
    expect(wrapper.get(".translation-direction").text()).toContain("自动识别");
    expect(wrapper.get(".translation-direction").text()).toContain("中文");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(backend.translateText).toHaveBeenCalledWith("你好，Hello", "zh-to-en");

    await input.setValue("Hello, world.");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(backend.translateText).toHaveBeenLastCalledWith("Hello, world.", "en-to-zh");
  });

  it("支持手动交换方向并恢复自动识别", async () => {
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("Hello");

    await wrapper.get(".translation-swap").trigger("click");
    expect(wrapper.get(".translation-direction").text()).toContain("手动方向");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(backend.translateText).toHaveBeenCalledWith("Hello", "zh-to-en");

    await action(wrapper, "恢复自动识别").trigger("click");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(backend.translateText).toHaveBeenLastCalledWith("Hello", "en-to-zh");
  });

  it("在前端拦截空白、纯符号和超过五千字符的内容", async () => {
    const wrapper = mountDialog();
    const input = wrapper.get(".translation-source textarea");

    await input.setValue("123 / ...");
    await action(wrapper, "翻译").trigger("click");
    expect(wrapper.get(".translation-error").text()).toContain("请输入中文或英文内容");

    await input.setValue("a".repeat(5001));
    await action(wrapper, "翻译").trigger("click");
    expect(wrapper.get(".translation-error").text()).toContain("不能超过 5000 个字符");
    expect(backend.translateText).not.toHaveBeenCalled();
  });

  it("翻译期间阻止重复请求，成功后展示并复制多个候选译文", async () => {
    let resolveRequest: (value: Array<{ label:string; text:string }>) => void = () => undefined;
    backend.translateText.mockReturnValueOnce(new Promise<Array<{ label:string; text:string }>>(resolve => { resolveRequest = resolve; }));
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("你好");

    const translateButton = action(wrapper, "翻译");
    await translateButton.trigger("click");
    await translateButton.trigger("click");
    expect(backend.translateText).toHaveBeenCalledOnce();
    expect(translateButton.text()).toBe("翻译中…");

    resolveRequest([
      { label: "常用译法", text: "Hello" },
      { label: "正式表达", text: "Greetings" },
      { label: "口语表达", text: "Hi" },
    ]);
    await flushPromises();
    expect(wrapper.findAll(".translation-candidate")).toHaveLength(3);
    expect(wrapper.get(".translation-result").text()).toContain("常用译法");
    expect(wrapper.get(".translation-result").text()).toContain("Greetings");
    await action(wrapper, "复制首选").trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Hello");
    expect(wrapper.get(".translation-dialog>footer").text()).toContain("已复制：常用译法");

    await wrapper.findAll(".translation-candidate .text-button")[1].trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("Greetings");
  });

  it("原文框按 Enter 翻译，Shift 加 Enter 保留换行", async () => {
    const wrapper = mountDialog();
    const input = wrapper.get(".translation-source textarea");
    await input.setValue("build");

    await input.trigger("keydown", { key:"Enter", shiftKey:true });
    expect(backend.translateText).not.toHaveBeenCalled();
    await input.trigger("keydown", { key:"Enter" });
    await flushPromises();
    expect(backend.translateText).toHaveBeenCalledWith("build", "en-to-zh");
    expect(wrapper.get(".translation-dialog>footer").text()).toContain("Shift + Enter 换行");
  });

  it("保留原始译文，四种命名文本点击后分别复制且不重复请求 AI", async () => {
    backend.translateText.mockResolvedValueOnce([{ label: "安全语境", text: "Hidden danger" }]);
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("隐患");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();

    expect(wrapper.get(".translation-original").text()).toBe("Hidden danger");
    const options = wrapper.findAll(".translation-naming-option");
    const expected = ["hiddenDanger", "HiddenDanger", "hidden_danger", "hidden-danger"];
    expect(options.map(option => option.get("code").text())).toEqual(expected);
    for (const [index, option] of options.entries()) {
      await option.trigger("click");
      await flushPromises();
      expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(expected[index]);
      expect(option.classes()).toContain("copied");
      expect(option.get(".translation-copy-state").text()).toBe("已复制 ✓");
      expect(wrapper.findAll(".translation-naming-option.copied")).toHaveLength(1);
    }
    await action(wrapper, "复制原译文").trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("Hidden danger");
    expect(backend.translateText).toHaveBeenCalledOnce();
  });

  it.each([
    ["API response ID", "apiResponseId", "ApiResponseId", "api_response_id", "api-response-id"],
    ["hiddenDanger", "hiddenDanger", "HiddenDanger", "hidden_danger", "hidden-danger"],
    ["  Hidden__danger-test  ", "hiddenDangerTest", "HiddenDangerTest", "hidden_danger_test", "hidden-danger-test"],
  ])("规范化英文短语的大小写和分隔符：%s", async (text, ...expected) => {
    backend.translateText.mockResolvedValueOnce([{ label: "译法", text }]);
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("隐患");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".translation-naming-option code").map(code => code.text())).toEqual(expected);
  });

  it.each(["隐患", "https://example.com", "const danger = true;", "Hidden\ndanger", "word ".repeat(13), "A".repeat(121), "42 danger"])("不为中文、长文本或代码生成命名格式：%s", async text => {
    backend.translateText.mockResolvedValueOnce([{ label: "译法", text }]);
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("隐患");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.find(".translation-naming-grid").exists()).toBe(false);
    expect(wrapper.get(".translation-original").text()).toBe(text.trim());
  });

  it("英译中不显示命名格式，即使译文包含英文专有名词", async () => {
    backend.translateText.mockResolvedValueOnce([{ label: "专有名词", text: "OpenAI" }]);
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("OpenAI");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.find(".translation-naming-grid").exists()).toBe(false);
  });

  it("复制失败不显示成功状态，关闭后清空命名候选和复制反馈", async () => {
    backend.translateText.mockResolvedValueOnce([{ label: "安全语境", text: "Hidden danger" }]);
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(new Error("剪贴板不可用"));
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("隐患");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    await wrapper.get(".translation-naming-option").trigger("click");
    await flushPromises();
    expect(wrapper.get(".translation-error").text()).toContain("复制失败");
    expect(wrapper.find(".translation-naming-option.copied").exists()).toBe(false);

    await wrapper.get(".translation-naming-option").trigger("click");
    await flushPromises();
    expect(wrapper.find(".translation-error").exists()).toBe(false);
    await wrapper.get("[aria-label='关闭翻译']").trigger("click");
    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    expect(wrapper.find(".translation-candidate").exists()).toBe(false);
    expect(wrapper.get("[role='status']").text()).toContain("Enter 翻译");
    expect(wrapper.find(".translation-error").exists()).toBe(false);
  });

  it("显示 DeepSeek 配置错误入口，并在关闭后清空临时内容", async () => {
    backend.translateText.mockRejectedValueOnce("尚未配置 DeepSeek API Key，请在设置中保存。");
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("你好");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.get(".translation-error").text()).toContain("尚未配置 DeepSeek API Key");
    expect(wrapper.get(".translation-error a").text()).toBe("前往设置");

    await wrapper.get(".translation-dialog>header .icon-button").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);
    expect(wrapper.get<HTMLTextAreaElement>(".translation-source textarea").element.value).toBe("");
  });

  it("浏览器演示模式不发送翻译请求", async () => {
    backend.isTauriRuntime.mockReturnValue(false);
    const wrapper = mountDialog();
    await wrapper.get(".translation-source textarea").setValue("Hello");
    await action(wrapper, "翻译").trigger("click");
    expect(wrapper.get(".translation-error").text()).toContain("需要在工作台桌面版中使用");
    expect(backend.translateText).not.toHaveBeenCalled();
  });

  it("报错翻译默认关闭，不显示发送提示，切换开关本身不发送请求", async () => {
    const wrapper = mountDialog();
    const toggle = wrapper.get<HTMLInputElement>("[role='switch']");
    expect(toggle.element.checked).toBe(false);
    expect(toggle.attributes("aria-checked")).toBe("false");
    expect(wrapper.find(".translation-privacy").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("输入内容才会发送给 DeepSeek");
    await wrapper.get("textarea").setValue("TypeError: invalid value");
    await toggle.setValue(true);
    expect(wrapper.find(".translation-direction").exists()).toBe(false);
    expect(wrapper.get<HTMLTextAreaElement>("textarea").element.value).toBe("TypeError: invalid value");
    expect(backend.translateText).not.toHaveBeenCalled();
    expect(backend.translateError).not.toHaveBeenCalled();
  });

  it("报错模式按 Enter 获取中文含义、可能原因和解决方法，并复制完整解读", async () => {
    const wrapper = mountDialog();
    await wrapper.get("[role='switch']").setValue(true);
    const input = wrapper.get("textarea");
    await input.setValue("  程序报错 TypeError: Cannot read properties of undefined (reading 'name')  ");
    await input.trigger("keydown", { key: "Enter", shiftKey: true });
    await input.trigger("keydown", { key: "Enter", isComposing: true });
    expect(backend.translateError).not.toHaveBeenCalled();
    await input.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(backend.translateError).toHaveBeenCalledWith("程序报错 TypeError: Cannot read properties of undefined (reading 'name')");
    expect(backend.translateText).not.toHaveBeenCalled();
    expect(wrapper.findAll(".translation-analysis-section h3").map(item => item.text())).toEqual(["报错含义", "可能原因", "排查与解决方法"]);
    expect(wrapper.get(".translation-result").text()).toContain(errorResult.meaning);
    expect(wrapper.get(".translation-result").text()).toContain(errorResult.possibleCauses[0]);
    expect(wrapper.get(".translation-result").text()).toContain(errorResult.solutions[1]);
    expect(wrapper.find(".translation-naming-grid").exists()).toBe(false);
    await action(wrapper, "复制解读").trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      `报错含义\n${errorResult.meaning}\n\n可能原因\n1. ${errorResult.possibleCauses[0]}\n\n排查与解决方法\n1. ${errorResult.solutions[0]}\n2. ${errorResult.solutions[1]}`,
    );
    expect(wrapper.get("[role='status']").text()).toBe("已复制：报错解读");
  });

  it("报错解读阻止重复请求，切回普通翻译后忽略迟到结果", async () => {
    let resolveRequest: (value: typeof errorResult) => void = () => undefined;
    backend.translateError.mockReturnValueOnce(new Promise(resolve => { resolveRequest = resolve; }));
    const wrapper = mountDialog();
    await wrapper.get("[role='switch']").setValue(true);
    await wrapper.get("textarea").setValue("TypeError");
    await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
    await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
    expect(backend.translateError).toHaveBeenCalledOnce();
    expect(action(wrapper, "翻译中…").attributes("disabled")).toBeDefined();
    await wrapper.get("[role='switch']").setValue(false);
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    resolveRequest(errorResult);
    await flushPromises();
    expect(wrapper.find(".translation-analysis-section").exists()).toBe(false);
    expect(wrapper.findAll(".translation-candidate")).toHaveLength(2);
    expect(wrapper.get(".translation-result").text()).not.toContain(errorResult.meaning);
  });

  it("普通翻译请求在切换到报错模式后不能覆盖解读结果", async () => {
    let resolveRequest: (value: Array<{ label: string; text: string }>) => void = () => undefined;
    backend.translateText.mockReturnValueOnce(new Promise(resolve => { resolveRequest = resolve; }));
    const wrapper = mountDialog();
    await wrapper.get("textarea").setValue("TypeError");
    await action(wrapper, "翻译").trigger("click");
    await wrapper.get("[role='switch']").setValue(true);
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    resolveRequest([{ label: "旧结果", text: "Old result" }]);
    await flushPromises();
    expect(wrapper.get(".translation-result").text()).toContain(errorResult.meaning);
    expect(wrapper.text()).not.toContain("Old result");
  });

  it("编辑或清空原文清除报错解读，关闭后开关恢复关闭", async () => {
    const wrapper = mountDialog();
    const toggle = wrapper.get<HTMLInputElement>("[role='switch']");
    await toggle.setValue(true);
    await wrapper.get("textarea").setValue("TypeError");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    await wrapper.get("textarea").setValue("SyntaxError");
    expect(wrapper.find(".translation-analysis-section").exists()).toBe(false);
    expect(action(wrapper, "复制解读").attributes("disabled")).toBeDefined();
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    await action(wrapper, "清空").trigger("click");
    expect(toggle.element.checked).toBe(true);
    expect(wrapper.get<HTMLTextAreaElement>("textarea").element.value).toBe("");
    expect(wrapper.find(".translation-analysis-section").exists()).toBe(false);
    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    expect(wrapper.get<HTMLInputElement>("[role='switch']").element.checked).toBe(false);
    expect(wrapper.find(".translation-direction").exists()).toBe(true);
  });

  it.each(["", "   ", "123 / ...", "a".repeat(5001)])("报错模式同样拦截无效原文（%#）", async text => {
    const wrapper = mountDialog();
    await wrapper.get("[role='switch']").setValue(true);
    await wrapper.get("textarea").setValue(text);
    await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
    expect(wrapper.find(".translation-error").exists()).toBe(true);
    expect(backend.translateError).not.toHaveBeenCalled();
  });

  it.each(["DeepSeek 请求失败：网络不可用", "DeepSeek 返回的报错解读不完整，请重试。", "尚未配置 DeepSeek API Key，请在设置中保存。"])("报错模式明确显示失败并允许重试：%s", async message => {
    backend.translateError.mockRejectedValueOnce(message);
    const wrapper = mountDialog();
    await wrapper.get("[role='switch']").setValue(true);
    await wrapper.get("textarea").setValue("TypeError");
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.get(".translation-error").text()).toContain(message);
    expect(wrapper.find(".translation-error a").exists()).toBe(message.includes("API Key"));
    expect(wrapper.find(".translation-analysis-section").exists()).toBe(false);
    await action(wrapper, "翻译").trigger("click");
    await flushPromises();
    expect(wrapper.find(".translation-error").exists()).toBe(false);
    expect(wrapper.findAll(".translation-analysis-section")).toHaveLength(3);
  });

  it("浏览器演示模式不发送报错解读请求", async () => {
    backend.isTauriRuntime.mockReturnValue(false);
    const wrapper = mountDialog();
    await wrapper.get("[role='switch']").setValue(true);
    await wrapper.get("textarea").setValue("TypeError");
    await action(wrapper, "翻译").trigger("click");
    expect(wrapper.get(".translation-error").text()).toContain("需要在工作台桌面版中使用");
    expect(backend.translateError).not.toHaveBeenCalled();
  });
});
