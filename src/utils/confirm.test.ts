import { afterEach, describe, expect, it } from "vitest";
import { confirmAction, resetConfirmQueueForTests, subscribeConfirm } from "./confirm";

afterEach(resetConfirmQueueForTests);

describe("workbench confirm service", () => {
  it("normalizes the dialog and resolves the selected result", async () => {
    const requests: Array<{ title:string; confirmText:string; respond:(accepted:boolean)=>void }> = [];
    subscribeConfirm((request) => requests.push(request));

    const result = confirmAction({ message:"确定发布吗？", confirmText:"开始发布", tone:"warning" });
    expect(requests).toHaveLength(1);
    expect(requests[0].title).toBe("请确认操作");
    expect(requests[0].confirmText).toBe("开始发布");
    requests[0].respond(true);
    await expect(result).resolves.toBe(true);
  });

  it("serializes multiple confirmation requests", async () => {
    const messages: string[] = [];
    const responders: Array<(accepted:boolean)=>void> = [];
    subscribeConfirm((request) => { messages.push(request.message); responders.push(request.respond); });

    const first = confirmAction("第一项");
    const second = confirmAction("第二项");
    expect(messages).toEqual(["第一项"]);
    responders[0](false);
    await expect(first).resolves.toBe(false);
    expect(messages).toEqual(["第一项","第二项"]);
    responders[1](true);
    await expect(second).resolves.toBe(true);
  });
});
