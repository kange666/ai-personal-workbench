import { afterEach, expect, it, vi } from "vitest";
import { discardMota50Frame, getMota50Frame, hideMota50Frame, positionMota50Frame } from "./mota50Frame";
afterEach(discardMota50Frame);
it("独立离线 iframe 在关闭和切换后仍然连接，画面裁切到弹窗内", () => {
  const frame = getMota50Frame();
  expect(getMota50Frame()).toBe(frame); expect(frame.getAttribute("src")).toBe("/rest/mota50/index.html");
  const message = vi.spyOn(frame.contentWindow!, "postMessage");
  positionMota50Frame(new DOMRect(20, -100, 640, 480), new DOMRect(0, 0, 800, 300));
  expect(frame.parentElement!.style.clipPath).toBe("inset(100px 0px 80px 0px)");
  hideMota50Frame(); expect(message).toHaveBeenCalled(); expect(frame.isConnected).toBe(true);
  positionMota50Frame(new DOMRect(20, 800, 640, 480), new DOMRect(0, 0, 800, 300));
  expect(frame.parentElement!.style.display).toBe("none");
  discardMota50Frame(); expect(frame.isConnected).toBe(false); expect(getMota50Frame()).not.toBe(frame);
});
