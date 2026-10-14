// @vitest-environment node
// These execute the bundled engine's logic and disk serialization, not a browser or a play-through.
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, mkdtempSync, writeFileSync, unlinkSync, rmdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import vm from "node:vm";
import { expect, it } from "vitest";
const root = "public/rest/mota50";
const source = name => readFileSync(`${root}/${name}`, "utf8");
const run = (context, code) => vm.runInContext(code, context);
function engine(storage = new Map(), failWrite = false) {
  const context = vm.createContext({ console, setTimeout, clearTimeout, URL, localforage: {
    setItem(name, value, callback) { if (failWrite) callback(new Error("quota")); else { storage.set(name, value); callback(null); } },
    getItem(name, callback) { callback(null, storage.get(name) ?? null); }
  } });
  run(context, "var window = globalThis; window.addEventListener=function(){}; var parent={}; var location={origin:'https://test.invalid',href:'https://test.invalid/rest/mota50/index.html'}; var main = { mode:'play', floors:{}, log:function(){} }; var core = { saves:{ids:{},cache:{}}, status:{} }; var flags = {}; var document = {addEventListener:function(){}};");
  for (const file of ["project/data.js", "project/functions.js", "project/enemys.js", "project/maps.js", "libs/utils.js", "libs/maps.js", "libs/enemys.js", "libs/events.js", "libs/control.js", "libs/thirdparty/lz-string.min.js"]) run(context, source(file));
  for (let floor = 0; floor <= 50; floor++) run(context, source(`project/floors/MT${floor}.js`));
  run(context, `
    var data = data_a1e2fb4a_e986_4524_b0da_9b7ba7c0874d;
    var functions = functions_d6ad677b_427a_4623_b50f_a445a3b0ef8a;
    core.utils = new utils(); core.maps = new maps(); core.enemys = new enemys();
    Object.getOwnPropertyNames(utils.prototype).forEach(function(name) {
      if (name !== 'constructor' && typeof core.utils[name] === 'function') core[name] = core.utils[name].bind(core.utils);
    });
    core.firstData = core.clone(data.firstData); core.firstData.name = 'astrion-mota50-v1';
    core.data = data; core.values = core.clone(data.values); core.flags = core.clone(data.flags);
    core.floorIds = data.main.floorIds; core.floors = main.floors; core.__SIZE__ = 13;
    core.material = { enemys: core.enemys.enemys, icons:{}, images:{ images:{} } };
    core.maps._initFloors();
    core.status = { floorId:'MT1', hard:'普通', hero:core.clone(data.firstData.hero), maps:core.maps.loadMap({}), route:[] };
    flags = core.status.hero.flags;
    core.getFlag = function(k,d) { return flags[k] == null ? d : flags[k]; };
    core.setFlag = function(k,v) { flags[k] = v; };
    core.removeFlag = function(k) { delete flags[k]; };
    core.hasFlag = function(k) { return !!flags[k]; };
    core.hasItem = function(k) { return !!(core.status.hero.items.constants[k] || core.status.hero.items.tools[k]); };
    core.hasSpecial = core.enemys.hasSpecial.bind(core.enemys);
    core.getStatusOrDefault = core.getRealStatusOrDefault = function(hero,k) { return hero && hero[k] != null ? hero[k] : core.status.hero[k]; };
    core.getEnemyValue = function(enemy,k) { return (typeof enemy === 'string' ? core.material.enemys[enemy] : enemy)[k]; };
    core.getGuid = function() { return 'anonymous-test'; };
    core.control = { _bindRoutePush: function() {} };
    core.saveData = functions.control.saveData;
    core.resetGame = function(hero,hard,floorId,maps,values) {
      core.status = {hero:hero,hard:hard,floorId:floorId,maps:maps,route:[]}; flags = hero.flags;
      core.values = Object.assign(core.clone(data.values), values);
    };
    core.events = {setVolume: function() {}};
    core.changeFloor = function(floorId,stair,loc,time,callback) { core.status.floorId=floorId; core.status.hero.loc=loc; callback(); };
    core.loadData = functions.control.loadData;
    core.playBgm = function() {};
    core.getMappedName = function(name) { return name; };
    main.loadJs = function(dir,list,callback) { callback(); }; main.init=function(){}; main.listen=function(){};
  `);
  run(context, source("workbench.js"));
  run(context, "main.loadJs('libs',[],function(){});");
  return context;
}

it("入口层和 1—50 层、剧情与怪物数据完整，上游数据逐字节保留", () => {
  const context = engine();
  expect(run(context, "core.floorIds.length")).toBe(51);
  for (let floor = 0; floor <= 50; floor++) {
    const id = `MT${floor}`;
    expect(run(context, `main.floors.${id}.floorId`)).toBe(id);
    const map = run(context, `main.floors.${id}.map`);
    expect(map).toHaveLength(13); for (const row of map) expect(row).toHaveLength(13);
  }
  const manifest = JSON.parse(source("upstream-manifest.json"));
  for (const [name, entry] of Object.entries(manifest.files)) {
    if (name === "index.html") continue;
    const bytes = readFileSync(`${root}/${name}`);
    expect(bytes.length, name).toBe(entry.bytes);
    expect(createHash("sha256").update(bytes).digest("hex"), name).toBe(entry.sha256);
  }
  expect(run(context, "JSON.stringify(main.floors.MT50)")).toContain("win");
});

it("所有配置的图片、地形、动画、音效与音乐均在本地，所有脚本语法有效", () => {
  const context = engine();
  const config = run(context, "data.main");
  const paths = [];
  for (const key of ["images", "tilesets", "bgms", "sounds"]) for (const name of config[key]) paths.push(`project/${key}/${name}`);
  for (const name of config.animates) paths.push(`project/animates/${name}.animate`);
  for (const name of ["animates", "enemys", "items", "npcs", "terrains", "enemy48", "npc48", "icons", "ground"]) paths.push(`project/materials/${name}.png`);
  for (const name of ["autotile", "autotile1", "autotile2", "autotile3"]) paths.push(`project/autotiles/${name}.png`);
  for (const name of paths) expect(readFileSync(`${root}/${name}`).length, name).toBeGreaterThan(0);
  function scripts(dir) { for (const entry of readdirSync(dir, {withFileTypes:true})) { const file = `${dir}/${entry.name}`; if (entry.isDirectory()) scripts(file); else if (file.endsWith(".js")) new vm.Script(readFileSync(file, "utf8"), {filename:file}); } }
  scripts(root);
  expect(source("index.html")).toContain("connect-src 'self'");
  expect(source("index.html")).toContain("<script src='workbench.js'>");
  expect(source("index.html")).not.toMatch(/<script(?![^>]*src=)[^>]*>/);
  for (const match of source("index.html").matchAll(/(?:src|href)=['"]([^'"]+)['"]/g)) {
    expect(readFileSync(`${root}/${match[1]}`).length, match[1]).toBeGreaterThan(0);
  }
});

it.each([1, 10, 20, 30, 40, 49, 50])("第 %i 层的角色、钥匙、道具、剧情标记和地图变化跨实例读档后完整恢复", floor => {
  const first = engine();
  run(first, `core.status.floorId='MT${floor}'; core.status.hero.hp=1234; core.status.hero.money=88;
    core.status.hero.items.tools.yellowKey=7; core.status.hero.items.constants.book=1;
    core.status.hero.flags.testStory='已经开门'; core.status.hero.loc={x:5,y:6,direction:'left'};
    core.status.maps.MT${floor}.map[6][5]=0; core.status.maps.MT1.map[1][1]=0;
    core.status.route=['up','left','item:book']; core.values.redPotion=77;
    var saved=core.saveData();`);
  const saved = JSON.parse(run(first, "JSON.stringify(saved)"));
  const directory = mkdtempSync(path.join(tmpdir(), "astrion-mota50-save-")), file = path.join(directory,"save.json");
  try {
    const storage = new Map();
    const writer = engine(storage); writer.saved = saved;
    run(writer, "var writeDone=false; core.setLocalForage('save1', saved, function(){writeDone=true;});");
    expect(run(writer,"writeDone")).toBe(true);
    writeFileSync(file, JSON.stringify([...storage]));
    const reader = engine(new Map(JSON.parse(readFileSync(file,"utf8"))));
    run(reader,"var loadDone=false; core.getLocalForage('save1',null,function(data){core.loadData(data,function(){loadDone=true;});});");
    expect(run(reader,"loadDone")).toBe(true);
    expect(run(reader,"core.status.floorId")).toBe(`MT${floor}`);
    expect(run(reader,"core.status.hero.hp")).toBe(1234);
    expect(run(reader,"core.status.hero.items.tools.yellowKey")).toBe(7);
    expect(run(reader,"core.status.hero.items.constants.book")).toBe(1);
    expect(run(reader,"core.status.hero.flags.testStory")).toBe("已经开门");
    expect(run(reader,"core.status.hero.loc.x")).toBe(5);
    expect(run(reader,`core.maps.decompressMap(core.status.maps.MT${floor}.map,'MT${floor}')[6][5]`)).toBe(0);
    expect(run(reader,"core.maps.decompressMap(core.status.maps.MT1.map,'MT1')[1][1]")).toBe(0);
    expect(run(reader,"core.values.redPotion")).toBe(77);
    expect(run(reader,"JSON.stringify(core.status.route)")).toBe('["up","left","item:book"]');
    expect([...storage.keys()]).toEqual(["astrion-mota50-v1_save1"]);
  } finally { try { unlinkSync(file); } finally { rmdirSync(directory); } }
});

it("多个存档槽互不覆盖，存储失败回调不会报告成功，损坏存档返回缺省值", () => {
  const storage = new Map(), context = engine(storage);
  run(context, "core.setLocalForage('save1',{hp:100}); core.setLocalForage('save2',{hp:200}); core.getLocalForage('save1',null,function(data){globalThis.readHP=data.hp;});");
  expect(run(context, "readHP")).toBe(100); expect(storage.size).toBe(2);
  const failed = engine(storage, true);
  run(failed, "var success=false, failure=false; core.setLocalForage('save1',{hp:0},function(){success=true;},function(){failure=true;});");
  expect(run(failed,"success")).toBe(false); expect(run(failed,"failure")).toBe(true);
  run(failed,"core.getLocalForage('save1',null,function(data){globalThis.oldHP=data.hp;});");
  expect(run(failed,"oldHP")).toBe(100);
  storage.set("astrion-mota50-v1_save3", "invalid-save");
  run(context,"core.getLocalForage('save3',null,function(data){globalThis.corrupt=data;});");
  expect(run(context,"corrupt")).toBeNull();
});

it("使用真实战斗规则：整除生命时最后一击无反击，无法破防不可战斗，高防零伤", () => {
  const context = engine();
  run(context,"core.status.hero.atk=11; core.status.hero.def=10; core.status.hero.mdef=0;");
  const damage = code => run(context, `functions.enemys.getDamageInfo(${code},null)`);
  const enemy = '{id:"greenSlime",hp:20,atk:18,def:1,special:0,money:1}';
  expect(damage(enemy).damage).toBe(8);
  run(context,"core.status.hero.atk=1;"); expect(damage(enemy)).toBeNull();
  run(context,"core.status.hero.atk=11; core.status.hero.def=100;"); expect(damage(enemy).damage).toBe(0);
});

it("原生存档槽写入、读档及自动存档落盘走通，重建实例后仍能恢复", () => {
  const storage = new Map(), first = engine(storage);
  const prepare = `core.ui={closePanel:function(){}}; core.status.event={interval:null}; core.isReplaying=function(){return false;};
    core.events.recoverEvents=function(){return false;}; core.playSound=function(){}; core.drawTip=function(text){globalThis.tip=text;};
    core.setLocalStorage=function(){}; core.saves.autosave={data:[],updated:false,storage:true,now:0}; core.animateFrame={};`;
  run(first, prepare);
  run(first,"core.status.hero.hp=321; control.prototype._doSL_save.call(core.control,2);");
  expect(run(first,"tip")).toBe("存档成功！");
  expect(storage.has("astrion-mota50-v1_save2")).toBe(true);
  const next = engine(storage); run(next,prepare);
  run(next,"control.prototype._doSL_load.call(core.control,2,function(id,data){ control.prototype._doSL_load_afterGet.call(core.control,id,data); });");
  expect(run(next,"core.status.hero.hp")).toBe(321); expect(run(next,"tip")).toBe("读档成功");
  run(first,"core.saves.autosave.data=[core.saveData()]; core.saves.autosave.now=1; core.saves.autosave.updated=true; control.prototype.checkAutosave.call(core.control);");
  expect(storage.has("astrion-mota50-v1_autoSave")).toBe(true);
});
