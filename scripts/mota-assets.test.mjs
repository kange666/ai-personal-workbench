// @vitest-environment node
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { expect, it } from "vitest";

it("保留原版游戏逐字节内容、570×410画幅和序章至21层的数据", () => {
  const file = readFileSync("public/rest/mota21/classic-21.swf");
  expect(createHash("sha256").update(file).digest("hex")).toBe("2f1681af86d28042009c7c1a43cfd9da8df20abcc598501db67175f31d87c646");
  expect(file.subarray(0, 3).toString()).toBe("CWS");
  const data = inflateSync(file.subarray(8));
  expect(data.length + 8).toBe(file.readUInt32LE(4));
  let bit = 0;
  function readBits(count) {
    let value = 0;
    for (let index = 0; index < count; index++, bit++) value = (value << 1) | ((data[bit >> 3] >> (7 - (bit & 7))) & 1);
    return value;
  }
  const size = readBits(5);
  const bounds = [readBits(size), readBits(size), readBits(size), readBits(size)];
  expect([(bounds[1] - bounds[0]) / 20, (bounds[3] - bounds[2]) / 20]).toEqual([570, 410]);
  const content = data.toString("utf8");
  for (let floor = 0; floor <= 21; floor++) expect(content).toContain(`mt_line_${String(floor).padStart(2, "0")}`);
  expect(content).toContain("冥灵魔王");
});

it("离线播放器的JS、WASM与许可证均打包齐全", () => {
  const folder = "public/rest/mota21/ruffle";
  const names = readdirSync(folder);
  expect(names).toContain("ruffle.js");
  expect(names).toContain("LICENSE_MIT");
  expect(names).toContain("LICENSE_APACHE");
  const scripts = names.filter(name => name.startsWith("core.ruffle.") && name.endsWith(".js"));
  const modules = names.filter(name => name.endsWith(".wasm"));
  expect(scripts).toHaveLength(2);
  expect(modules).toHaveLength(2);
  for (const name of modules) expect([...readFileSync(`${folder}/${name}`).subarray(0, 4)]).toEqual([0, 97, 115, 109]);
});
