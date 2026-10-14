// @vitest-environment node
import vm from "node:vm";
import { readFileSync } from "node:fs";
import { afterEach, expect, it, vi } from "vitest";
const source = readFileSync("public/rest/mota50/workbench-clock.js", "utf8");
afterEach(() => vi.useRealTimers());
function clock() {
  vi.useFakeTimers(); let time=0;
  const context = vm.createContext({performance:{now:()=>time},setTimeout,clearTimeout,requestAnimationFrame:cb=>setTimeout(()=>cb(time),16),cancelAnimationFrame:clearTimeout});
  vm.runInContext("var window=globalThis",context); vm.runInContext(source,context);
  return { context, advance(ms) { time+=ms; vi.advanceTimersByTime(ms); }, run:code=>vm.runInContext(code,context) };
}
it("暂停冻结延迟与循环任务，恢复后保留剩余时间，不补跑后台计时", () => {
  const c=clock(); c.run("var calls=0; var interval=setInterval(function(){calls++},100); var done=false; setTimeout(function(){done=true},250);");
  c.advance(60); c.run("MotaWorkbenchClock.pause()"); c.advance(10000);
  expect(c.run("calls")).toBe(0); expect(c.run("done")).toBe(false);
  c.run("MotaWorkbenchClock.resume()"); c.advance(40); expect(c.run("calls")).toBe(1);
  c.advance(150); expect(c.run("done")).toBe(true); expect(c.run("calls")).toBe(2);
  c.run("clearInterval(interval)"); c.advance(1000); expect(c.run("calls")).toBe(2);
});
it("暂停期间新任务可取消，帧时间扣除暂停时长且重复恢复不重复调度", () => {
  const c=clock(); c.run("MotaWorkbenchClock.pause(); var calls=0; var id=setTimeout(function(){calls++},1); clearTimeout(id); var stamp=-1; requestAnimationFrame(function(t){stamp=t});");
  c.advance(10000); expect(c.run("stamp")).toBe(-1);
  c.run("MotaWorkbenchClock.resume(); MotaWorkbenchClock.resume()"); c.advance(16);
  expect(c.run("calls")).toBe(0); expect(c.run("stamp")).toBe(16);
});
