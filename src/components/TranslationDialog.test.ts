import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(() => true),
  translateText: vi.fn(),
}));

vi.mock("../services/backend", () => backend);

import TranslationDialog from "./TranslationDialog.vue";

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
});
