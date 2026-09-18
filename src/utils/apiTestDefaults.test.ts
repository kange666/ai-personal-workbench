import { describe, expect, it } from "vitest";
import { apiTestDefault, paginationDefault } from "./apiTestDefaults";

describe("接口测试初始值", () => {
  it("识别分页字段并保留文档默认值", () => {
    expect(paginationDefault("pageNum")).toBe(1);
    expect(paginationDefault("page_size")).toBe(10);
    expect(paginationDefault("offset")).toBe(0);
    expect(paginationDefault("departmentSize")).toBeUndefined();
    expect(apiTestDefault({type:"integer",default:20},"pageSize")).toBe(20);
    expect(apiTestDefault({type:"integer",default:0},"pageIndex")).toBe(0);
    expect(apiTestDefault({type:"boolean",default:false},"enabled")).toBe(false);
  });
  it("按示例、枚举和字段类型生成必填值", () => {
    expect(apiTestDefault({type:"string",example:"示例名称"},"name")).toBe("示例名称");
    expect(apiTestDefault({type:"string",enum:["ready","closed"]},"status")).toBe("ready");
    expect(apiTestDefault({type:"integer",minimum:5},"count")).toBe(5);
    expect(apiTestDefault({type:"string",format:"email"},"email")).toBe("test@example.com");
    expect(apiTestDefault({type:"object",required:["id"],properties:{id:{type:"integer"},optional:{type:"string"}}},"data")).toEqual({id:1});
  });
});
