// @vitest-environment node
// Integration protocol tests with an in-process engine stub; no website is accessed.
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { expect, it } from "vitest";
function bridge() {
  const context = vm.createContext({URL});
  vm.runInContext(`
    var window=globalThis, handlers={}, notices=[], calls=[];
    var location={origin:'https://test.invalid',href:'https://test.invalid/rest/mota50/index.html'};
    var parent={postMessage:function(message){notices.push(message)}};
    window.addEventListener=function(type,handler){handlers[type]=handler;};
    var document={addEventListener:function(){},getElementById:function(){return null;}};
    window.MotaWorkbenchClock={pause:function(){calls.push('clock-pause')},resume:function(){calls.push('clock-resume')}};
    function utils(){}; utils.prototype.http=function(){calls.push('http')}; utils.prototype._setLocalForage_set=function(){};
    function events(){}; function control(){};
    var data_a1e2fb4a_e986_4524_b0da_9b7ba7c0874d={firstData:{name:'51'}};
    var core={status:{event:{id:null},holdingKeys:[38]},saves:{cache:{}},musicStatus:{},
      stopAutomaticRoute:function(){calls.push('stop')}, control:{checkAutosave:function(){calls.push('autosave')}},
      pauseBgm:function(){calls.push('mute')},stopSound:function(){},resumeBgm:function(keepTime){calls.push(keepTime?'unmute-continue':'unmute-restart')},
      save:function(){calls.push('save')},load:function(){calls.push('load')},isPlaying:function(){return true;}};
    var main={loadJs:function(dir,list,callback){callback()}, init:function(mode,callback){window.finish=callback},listen:function(){}};
  `, context);
  vm.runInContext(readFileSync("public/rest/mota50/workbench.js","utf8"),context);
  return {run:code=>vm.runInContext(code,context)};
}
it("只接收同来源父窗口指令，准备好前不运行；恢复、存读档和暂停通知生效", () => {
  const b=bridge();
  b.run("handlers.message({source:parent,origin:location.origin,data:{channel:'astrion-mota50-v1',type:'save'}})");
  expect(b.run("calls.length")).toBe(0);
  b.run("finish()"); expect(b.run("calls.includes('clock-pause')")).toBe(true);
  b.run("handlers.message({source:{},origin:location.origin,data:{channel:'astrion-mota50-v1',type:'resume'}})");
  expect(b.run("calls.includes('clock-resume')")).toBe(false);
  for (const type of ["resume","save","load","pause"]) b.run(`handlers.message({source:parent,origin:location.origin,data:{channel:'astrion-mota50-v1',type:'${type}'}})`);
  expect(b.run("calls.includes('clock-resume') && calls.includes('save') && calls.includes('load')")).toBe(true);
  expect(b.run("calls.includes('unmute-continue') && !calls.includes('unmute-restart')")).toBe(true);
  expect(b.run("core.status.holdingKeys.length")).toBe(0);
  b.run("handlers.message({source:parent,origin:'https://other.invalid',data:{channel:'astrion-mota50-v1',type:'resume'}})");
  expect(b.run("calls[calls.length-1]")).toBe("clock-pause");
});
it("Tab/外层 Esc 传递给弹窗，游戏内部菜单的 Esc 留给原生游戏", () => {
  const b=bridge(); b.run("finish()");
  b.run("var prevented=0; function key(key){handlers.keydown({key:key,shiftKey:true,preventDefault:function(){prevented++},stopImmediatePropagation:function(){}})}; key('Tab'); key('Escape');");
  expect(b.run("notices.map(function(message){return message.type}).join(',')")).toBe("ready,tab,escape");
  b.run("core.status.event.id='save'; key('Escape');");
  expect(b.run("notices.length")).toBe(3);
});
it("存档空间独立，只准读取本地资源，拒绝统计上传和云存档请求", () => {
  const b=bridge(); b.run("main.loadJs('project',[],function(){});main.loadJs('libs',[],function(){});");
  expect(b.run("data_a1e2fb4a_e986_4524_b0da_9b7ba7c0874d.firstData.name")).toBe("astrion-mota50-v1");
  b.run("utils.prototype.http('GET','project/data.js');utils.prototype.http('GET','https://other.invalid/data.js');utils.prototype.http('POST','/games/upload.php');utils.prototype.http('GET','/private');");
  expect(b.run("calls.filter(function(type){return type==='http'}).length")).toBe(1);
});
