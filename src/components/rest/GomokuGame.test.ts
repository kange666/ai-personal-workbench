import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import GomokuGame from "./GomokuGame.vue";
import { gameSaveKey } from "../../composables/useGameSave";
import { isGomokuState } from "../../utils/rest/gomoku";

const loop = vi.hoisted(() => ({ tick: (_dt: number) => {}, paused: (): boolean => false }));
const network = vi.hoisted(() => ({ move: vi.fn() }));
vi.mock("../../services/backend", () => ({ getGomokuDeepSeekMove: network.move }));
vi.mock('../../utils/rest/gomokuExpert',async importOriginal=>({...await importOriginal<typeof import('../../utils/rest/gomokuExpert')>(),searchGomokuDeepSeek:vi.fn(()=>({index:113}))}));
vi.mock("../../composables/useGameLoop", () => ({ useGameLoop: (paused: () => boolean, tick: (dt: number) => void) => { loop.tick = tick; loop.paused = paused; } }));
let wrapper: VueWrapper | undefined;
const stored = () => JSON.parse(localStorage.getItem(gameSaveKey("gomoku"))!);
async function advance() { for (let i = 0; i < 10; i += 1) if (!loop.paused()) loop.tick(0.05); await nextTick(); }

beforeEach(() => { localStorage.clear(); network.move.mockReset(); });
afterEach(() => { wrapper?.unmount(); wrapper = undefined; vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("五子棋界面与电脑回合", () => {
  const button = (text:string) => wrapper!.findAll('button').find(b=>b.text()===text)!;
  async function startDeepSeek() {
    wrapper=mount(GomokuGame,{props:{paused:false}});
    await wrapper.get('[aria-label="电脑难度"]').setValue('deepseek');
    await button('开始 DeepSeek 对弈').trigger('click');
  }
  it('DeepSeek 需主动启用，只发送棋盘和执子，收到真实结果后只落一次', async () => {
    network.move.mockResolvedValue({index:113,model:'test-model'});
    wrapper=mount(GomokuGame,{props:{paused:false}});
    await wrapper.get('[aria-label="电脑难度"]').setValue('deepseek');
    await wrapper.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    expect(network.move).not.toHaveBeenCalled();expect(stored().state.run.moves).toHaveLength(0);
    expect(wrapper.text()).toContain('只发送棋盘和执子');
    await button('开始 DeepSeek 对弈').trigger('click');
    await wrapper.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();await flushPromises();
    expect(network.move).toHaveBeenCalledTimes(1);
    expect(network.move.mock.calls[0]).toEqual([Array.from({length:225},(_,i)=>i===112?1:0),2]);
    expect(stored().state.run.moves).toEqual([{index:112,player:1},{index:113,player:2}]);
    expect(stored().state.difficulty).toBe('deepseek');expect(isGomokuState(stored().state.run)).toBe(true);
    await advance();expect(network.move).toHaveBeenCalledTimes(1);
  });
  it('DeepSeek 失败不自动代下、不循环请求，手动重试后恢复', async () => {
    network.move.mockRejectedValueOnce('DeepSeek 请求失败或超时，请重试。').mockResolvedValueOnce({index:113,model:'test'});
    await startDeepSeek();await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});
    await advance();await flushPromises();await advance();
    expect(stored().state.run.moves).toHaveLength(1);expect(network.move).toHaveBeenCalledTimes(1);
    expect(wrapper!.text()).toContain('DeepSeek 请求失败');
    await button('重试思考').trigger('click');await advance();await flushPromises();
    expect(network.move).toHaveBeenCalledTimes(2);expect(stored().state.run.moves).toHaveLength(2);
  });
  it('模型无合法建议时明确提示最高档修正，修正后仍可正常继续下一回合',async()=>{
    network.move.mockResolvedValue({index:null,model:'test',notice:'DeepSeek 未给出合法建议，本步由最高档搜索修正。'});
    await startDeepSeek();await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});
    await advance();await flushPromises();
    expect(stored().state.run.moves).toHaveLength(2);
    expect(wrapper!.text()).toContain('本步由最高档搜索修正');
    expect(wrapper!.findAll('button').some(b=>b.text()==='重试思考')).toBe(false);
    await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'ArrowDown'});
    await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();await flushPromises();
    expect(network.move).toHaveBeenCalledTimes(2);
  });
  it('悔棋或切换难度后，DeepSeek 的迟到结果不能污染棋局', async () => {
    let resolve!:(value:{index:number;model:string})=>void;
    network.move.mockImplementation(()=>new Promise(r=>{resolve=r;}));
    await startDeepSeek();await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    await button('悔一步').trigger('click');resolve({index:113,model:'test'});await flushPromises();await advance();
    expect(stored().state.run.moves).toHaveLength(0);expect(network.move).toHaveBeenCalledTimes(1);
    await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    await wrapper!.get('[aria-label="电脑难度"]').setValue('normal');
    await advance();expect(stored().state.run.moves).toHaveLength(2);
    resolve({index:114,model:'test'});await flushPromises();
    expect(stored().state.run.moves).toHaveLength(2);
  });
  it('暂停期间不接收联网落子，恢复后可继续；重新打开仍需主动启用', async () => {
    let resolve!:(value:{index:number;model:string})=>void;
    network.move.mockImplementation(()=>new Promise(r=>{resolve=r;}));
    await startDeepSeek();await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    await wrapper!.setProps({paused:true});resolve({index:113,model:'test'});await flushPromises();
    expect(stored().state.run.moves).toHaveLength(1);
    await wrapper!.setProps({paused:false});await advance();expect(network.move).toHaveBeenCalledTimes(2);
    wrapper!.unmount();resolve({index:114,model:'test'});await flushPromises();
    wrapper=mount(GomokuGame,{props:{paused:false}});await advance();
    expect(stored().state.run.moves).toHaveLength(1);expect(network.move).toHaveBeenCalledTimes(2);
    network.move.mockResolvedValue({index:113,model:'test'});
    await button('开始 DeepSeek 对弈').trigger('click');await advance();await flushPromises();
    expect(stored().state.run.moves).toHaveLength(2);
  });
  it('拒绝占用格、越界和非整数落点，不用本地AI冒充', async () => {
    for(const index of [112,225,1.5]) {
      network.move.mockResolvedValue({index,model:'test'});
      if(!wrapper)await startDeepSeek();
      else await button('重试思考').trigger('click');
      if(stored().state.run.moves.length===0)await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});
      await advance();await flushPromises();
      expect(stored().state.run.moves).toHaveLength(1);expect(wrapper!.text()).toContain('无效落点');
    }
  });
  it('最高档把联网建议交给独立线程深搜，暂停可取消且不会锁住后续对局',async()=>{
    const workers: {onmessage:((event:{data:{index:number}})=>void)|null;terminate:ReturnType<typeof vi.fn>;postMessage:ReturnType<typeof vi.fn>}[]=[];
    vi.stubGlobal('Worker',class{onmessage=null;onerror=null;terminate=vi.fn();postMessage=vi.fn();constructor(){workers.push(this);}});
    network.move.mockResolvedValue({index:0,model:'test'});
    await startDeepSeek();await wrapper!.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();await flushPromises();
    expect(workers[0]!.postMessage.mock.calls[0]![0]).toMatchObject({side:2,strongest:true,preferred:0});
    await wrapper!.setProps({paused:true});await flushPromises();expect(workers[0]!.terminate).toHaveBeenCalled();
    workers[0]!.onmessage?.({data:{index:113}});expect(stored().state.run.moves).toHaveLength(1);
    await wrapper!.setProps({paused:false});await advance();await flushPromises();
    workers[1]!.onmessage?.({data:{index:113}});await flushPromises();
    expect(stored().state.run.board[0]).toBe(0);expect(stored().state.run.board[113]).toBe(2);
    expect(network.move).toHaveBeenCalledTimes(2);
  });
  it('高手在独立线程计算，暂停、悔棋和重开后丢弃过期结果', async () => {
    const workers: { onmessage: ((event:{data:{index:number}})=>void)|null; terminate: ReturnType<typeof vi.fn>; postMessage: ReturnType<typeof vi.fn> }[]=[];
    vi.stubGlobal('Worker',class { onmessage=null;onerror=null;terminate=vi.fn();postMessage=vi.fn();constructor(){workers.push(this);} });
    wrapper=mount(GomokuGame,{props:{paused:false}});
    await wrapper.get('[aria-label="电脑难度"]').setValue('expert');
    await wrapper.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    expect(workers).toHaveLength(1);expect(workers[0]!.postMessage.mock.calls[0]![0].board[112]).toBe(1);
    await wrapper.setProps({paused:true});expect(workers[0]!.terminate).toHaveBeenCalled();
    workers[0]!.onmessage?.({data:{index:113}});expect(stored().state.run.moves).toHaveLength(1);
    await wrapper.setProps({paused:false});await advance();expect(workers).toHaveLength(2);
    await wrapper.findAll('button').find(b=>b.text()==='悔一步')!.trigger('click');
    workers[1]!.onmessage?.({data:{index:113}});expect(stored().state.run.moves).toHaveLength(0);
    await wrapper.get('[data-game-focus]').trigger('keydown',{key:'Enter'});await advance();
    workers[2]!.onmessage?.({data:{index:113}});await nextTick();expect(stored().state.run.moves).toHaveLength(2);
    expect(isGomokuState(stored().state.run)).toBe(true);
  });
  it("方向键和空格落子，电脑仅回应一次，悔棋回到玩家回合", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    const surface = wrapper.get("[data-game-focus]");
    await surface.trigger("keydown", { key: "ArrowRight" });
    await surface.trigger("keydown", { key: " " });
    expect(stored().state.run.moves).toEqual([{ index: 113, player: 1 }]);
    expect(wrapper.text()).toContain("电脑正在思考");
    await advance();
    await advance();
    expect(stored().state.run.moves).toHaveLength(2);
    expect(wrapper.text()).toContain("轮到你了");
    expect(isGomokuState(stored().state.run)).toBe(true);
    await wrapper.get("button").trigger("click");
    expect(stored().state.run.moves).toHaveLength(0);
    await advance();
    expect(stored().state.run.moves).toHaveLength(0);
  });

  it("关闭于电脑回合，恢复后继续思考且不会重复落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: "Enter" });
    wrapper.unmount();
    wrapper = mount(GomokuGame, { props: { paused: true } });
    await advance();
    expect(stored().state.run.moves).toHaveLength(1);
    await wrapper.setProps({ paused: false });
    await advance();
    await advance();
    expect(stored().state.run.moves).toHaveLength(2);
  });

  it("暂停阻止落子和悔棋，长按空格不落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: true } });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    expect(wrapper.text()).toContain("你的步数 0");
    await wrapper.setProps({ paused: false });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " ", repeat: true });
    expect(wrapper.text()).toContain("你的步数 0");
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    await wrapper.setProps({ paused: true });
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
  });

  it("鼠标点击按缩放后的棋盘坐标落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    const board = wrapper.get("svg");
    vi.spyOn(board.element, "getBoundingClientRect").mockReturnValue({ x: 20, y: 40, left: 20, top: 40, right: 380, bottom: 400, width: 360, height: 360, toJSON: () => ({}) });
    await board.trigger("click", { clientX: 20 + 19 + 23 * 3, clientY: 40 + 19 + 23 * 5 });
    expect(stored().state.run.moves[0]).toEqual({ index: 78, player: 1 });
  });
});
