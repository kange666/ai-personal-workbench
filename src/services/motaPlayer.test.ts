import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { MotaPlayer } from "./motaPlayer";

beforeEach(() => { vi.resetModules(); document.body.innerHTML = ""; document.querySelectorAll('script[src*="mota21"]').forEach(script => script.remove()); });
afterEach(() => { document.body.innerHTML = ""; vi.unstubAllGlobals(); });

async function start(load = vi.fn(async () => {})) {
  const service = await import("./motaPlayer");
  const suspend = vi.fn(), resume = vi.fn();
  const createPlayer = vi.fn(() => Object.assign(document.createElement("div"), { ruffle: () => ({ load, suspend, resume }) }) as MotaPlayer);
  const pending = service.getMotaPlayer();
  Object.assign(window, { RufflePlayer: { newest: () => ({ createPlayer }) } });
  document.querySelector('script[src*="mota21"]')!.dispatchEvent(new Event("load"));
  return { service, pending, load, suspend, resume, createPlayer };
}

it("离线资源加载一次，关闭时不移除播放器，重新打开复用原运行状态", async () => {
  const { service, pending, load, createPlayer } = await start();
  const player = await pending;
  service.positionMotaPlayer({ left: 100, top: 80, width: 570, height: 410 });
  expect(player.isConnected).toBe(true);
  service.hideMotaPlayer();
  expect(player.isConnected).toBe(true);
  expect(player.parentElement!.style.display).toBe("none");
  expect(player.parentElement!.inert).toBe(true);
  expect(await service.getMotaPlayer()).toBe(player);
  expect(createPlayer).toHaveBeenCalledOnce();
  expect(load).toHaveBeenCalledWith(expect.objectContaining({ url: "/rest/mota21/classic-21.swf", allowScriptAccess: false, allowNetworking: "none", openUrlMode: "deny" }));
  service.positionMotaPlayer({ left: 30, top: 25, width: 285, height: 205 });
  expect(player.parentElement!.style.width).toBe("285px");
  expect(player.parentElement!.inert).toBe(false);
  service.discardMotaPlayer();
  expect(player.isConnected).toBe(false);
});

it("原文件加载失败清理节点，可以再次加载", async () => {
  const load = vi.fn().mockRejectedValueOnce(new Error("读取失败")).mockResolvedValue(undefined);
  const { service, pending } = await start(load);
  await expect(pending).rejects.toThrow("读取失败");
  expect(document.querySelector(".mota-player-host")).toBeNull();
  const player = await service.getMotaPlayer();
  expect(player.isConnected).toBe(true);
  expect(load).toHaveBeenCalledTimes(2);
  service.discardMotaPlayer();
});

it("滚动时仅显示弹窗内的画面，裁切不改变舞台尺寸，滚回后复用进度", async () => {
  const { service, pending } = await start();
  const player = await pending;
  const host = player.parentElement!;
  const bounds = { left: 130, top: 100, width: 500, height: 230 };
  service.positionMotaPlayer({ left: 100, top: -20, width: 570, height: 410 }, bounds);
  expect(host.style.clipPath).toBe("inset(120px 40px 60px 30px)");
  expect(host.style.width).toBe("570px");
  expect(host.style.height).toBe("410px");

  service.positionMotaPlayer({ left: 100, top: -410, width: 570, height: 410 }, bounds);
  expect(host.style.display).toBe("none");
  expect(host.inert).toBe(true);
  expect(player.isConnected).toBe(true);

  service.positionMotaPlayer({ left: 130, top: 100, width: 500, height: 230 }, bounds);
  expect(host.style.display).toBe("block");
  expect(host.style.clipPath).toBe("inset(0px 0px 0px 0px)");
  expect(host.inert).toBe(false);
  expect(await service.getMotaPlayer()).toBe(player);
  service.discardMotaPlayer();
});

it("加载期间重开，旧请求失败不会覆盖新播放器", async () => {
  let fail!: (error: Error) => void;
  const load = vi.fn().mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject; })).mockResolvedValue(undefined);
  const { service, pending } = await start(load);
  const result = pending.catch(error => error);
  await Promise.resolve(); await Promise.resolve();
  service.discardMotaPlayer();
  const player = await service.getMotaPlayer();
  fail(new Error("旧请求已取消"));
  await result;
  expect(await service.getMotaPlayer()).toBe(player);
  expect(document.querySelectorAll(".mota-player-host")).toHaveLength(1);
  service.discardMotaPlayer();
});

it("运行库网络错误可重试，不把错误当作加载完成", async () => {
  const service = await import("./motaPlayer");
  const pending = service.getMotaPlayer();
  const rejected = expect(pending).rejects.toThrow("播放器加载失败");
  document.querySelector('script[src*="mota21"]')!.dispatchEvent(new Event("error"));
  await rejected;
  const { pending: retry, service: retryService } = await start();
  expect((await retry).isConnected).toBe(true);
  retryService.discardMotaPlayer();
});
