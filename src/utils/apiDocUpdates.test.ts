import { beforeEach, describe, expect, it } from "vitest";
import { observeApiUpdates, readApiUpdates, saveApiUpdates } from "./apiDocUpdates";

describe("接口更新已读状态",()=>{
  beforeEach(()=>localStorage.clear());
  it("首次建立基线，内容变化和新增才提示，未读跨多次同步保留",()=>{
    const initial=observeApiUpdates(null,[{id:"a",documentHash:"1"}]);
    expect(initial.unread).toEqual({});
    const changed=observeApiUpdates(initial,[{id:"a",documentHash:"2"},{id:"b",documentHash:"3"}]);
    expect(changed.unread).toEqual({a:"updated",b:"added"});
    expect(observeApiUpdates(changed,[{id:"a",documentHash:"2"},{id:"b",documentHash:"3"}]).unread).toEqual(changed.unread);
    delete changed.unread.a;
    saveApiUpdates("source",changed);
    const reopened=observeApiUpdates(readApiUpdates("source"),[{id:"a",documentHash:"2"}]);
    expect(reopened.unread).toEqual({});
    expect(reopened.known).toEqual({a:"2"});
    expect(observeApiUpdates(reopened,[{id:"a",documentHash:"4"}]).unread).toEqual({a:"updated"});
    expect(readApiUpdates("other-source")).toBeNull();
  });
  it("空项目首次同步显示新增，损坏存储安全降级",()=>{
    expect(observeApiUpdates(observeApiUpdates(null,[]),[{id:"a",documentHash:"1"}]).unread).toEqual({a:"added"});
    localStorage.setItem("workbench-api-doc-updates-v1:source","not-json");
    expect(readApiUpdates("source")).toBeNull();
  });
});
