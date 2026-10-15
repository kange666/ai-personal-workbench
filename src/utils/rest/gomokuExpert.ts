import type { Stone } from './gomoku';

type Side = 1 | 2;
const axes = [[1, 0], [0, 1], [1, 1], [1, -1]] as const;
const MATE = 100_000_000;
const inside = (x: number, y: number) => x >= 0 && y >= 0 && x < 15 && y < 15;
const other = (side: Side): Side => side === 1 ? 2 : 1;
export interface GomokuThreat { score: number; five: boolean; wins: number[]; threes: number; fours: number }
interface LinePattern { five:boolean; wins:number[]; three:boolean; sleeping:boolean; two:boolean }
// 相同线形只识别一次；最多缓存三万种11格线形，避免深搜反复创建匹配对象。
const patternCache=new Map<string,LinePattern>();
function linePattern(line:string):LinePattern {
  const cached=patternCache.get(line);if(cached)return cached;
  let five=false,sleeping=false;const wins=new Set<number>();
  for(let start=1;start<=5;start++){
    const section=line.slice(start,start+5);if(section.includes('#'))continue;
    const count=section.split('1').length-1;
    if(count===5)five=true;if(count===4)wins.add(start+section.indexOf('0')-5);if(count===3)sleeping=true;
  }
  const contains=(patterns:string[])=>patterns.some(pattern=>{
    for(let start=line.indexOf(pattern);start>=0;start=line.indexOf(pattern,start+1))if(start<=5&&start+pattern.length>5)return true;
    return false;
  });
  const result={five,wins:[...wins],three:!wins.size&&contains(['01110','010110','011010']),sleeping,two:contains(['001100','0010100','01010','010010'])};
  if(patternCache.size>=30_000)patternCache.clear();patternCache.set(line,result);return result;
}

/** 以待落点为中心识别连四、跳四、活三及跳活三，边界和对方棋子均视为封口。 */
export function gomokuThreat(board: readonly Stone[], index: number, side: Side): GomokuThreat {
  let five = false, threes = 0, fours = 0, sleeping = 0, twos = 0;
  const wins = new Set<number>(), x = index % 15, y = Math.floor(index / 15);
  for (const [dx, dy] of axes) {
    let line = '';
    for (let d = -5; d <= 5; d++) {
      const nx = x + dx * d, ny = y + dy * d, cell = ny * 15 + nx;
      line += d === 0 ? '1' : !inside(nx, ny) ? '#' : board[cell] === side ? '1' : board[cell] ? '#' : '0';
    }
    const pattern=linePattern(line);five ||= pattern.five;
    pattern.wins.forEach(offset=>wins.add(index+offset*(dy*15+dx)));
    if(pattern.wins.length)fours++;
    if(pattern.three)threes++;else if(!pattern.wins.length&&pattern.sleeping)sleeping++;
    if(pattern.two)twos++;
  }
  const score = five ? MATE : wins.size >= 2 ? 2_000_000 : fours && threes ? 240_000
    : threes >= 2 ? 100_000 : fours ? 45_000 : threes ? 9_000 : 0;
  return { five, wins: [...wins], threes, fours, score: score + threes * 900 + sleeping * 240 + twos * 65 };
}

export function gomokuCandidates(board: readonly Stone[]) {
  const near = new Set<number>();
  for (let index = 0; index < 225; index++) if (board[index]) {
    const x = index % 15, y = Math.floor(index / 15);
    for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) {
      const nx = x + dx, ny = y + dy, cell = ny * 15 + nx;
      if (inside(nx, ny) && board[cell] === 0) near.add(cell);
    }
  }
  return near.size ? [...near] : board[112] === 0 ? [112] : [];
}

interface Ranked { index: number; own: GomokuThreat; enemy: GomokuThreat; value: number }
function rank(board: Stone[], side: Side): Ranked[] {
  const opponent = other(side);
  return gomokuCandidates(board).map(index => {
    const own = gomokuThreat(board, index, side), enemy = gomokuThreat(board, index, opponent);
    const distance = Math.abs(index % 15 - 7) + Math.abs(Math.floor(index / 15) - 7);
    return { index, own, enemy, value: Math.max(own.score * 1.08, enemy.score) + Math.min(own.score, enemy.score) * .2 - distance };
  }).sort((a, b) => b.value - a.value || a.index - b.index);
}

export interface ExpertOptions { timeMs?: number; maxDepth?: number; maxNodes?: number; forcingDepth?: number; preferred?: number; incremental?: boolean; rootCandidates?: number[] }
// 最高档只重算落子影响到的四条线；撤子时恢复父局面的缓存，给真正的深搜留出预算。
class IncrementalPosition {
  private cached: Array<[GomokuThreat, GomokuThreat] | undefined> = Array(225);
  private history: Array<Array<[number, [GomokuThreat, GomokuThreat] | undefined]>> = [];
  constructor(private board: Stone[]) {}
  play(index:number,side:Side) {
    const x=index%15,y=Math.floor(index/15),changed=new Set([index]);
    for(const [dx,dy] of axes)for(let d=-5;d<=5;d++)if(inside(x+dx*d,y+dy*d))changed.add(index+d*(dy*15+dx));
    this.history.push([...changed].map(cell=>[cell,this.cached[cell]]));
    for(const cell of changed)this.cached[cell]=undefined;
    this.board[index]=side;
  }
  undo(index:number) {
    this.board[index]=0;
    for(const [cell,entry] of this.history.pop()!)this.cached[cell]=entry;
  }
  rank(side:Side):Ranked[] {
    return gomokuCandidates(this.board).map(index=>{
      const entry=this.cached[index]??(this.cached[index]=[gomokuThreat(this.board,index,1),gomokuThreat(this.board,index,2)]);
      const own=entry[side-1]!,enemy=entry[other(side)-1]!;
      const distance=Math.abs(index%15-7)+Math.abs(Math.floor(index/15)-7);
      return {index,own,enemy,value:Math.max(own.score*1.08,enemy.score)+Math.min(own.score,enemy.score)*.2-distance};
    }).sort((a,b)=>b.value-a.value||a.index-b.index);
  }
}
interface Search { deadline: number; maxNodes: number; nodes: number; stopped: boolean; position?:IncrementalPosition }
const rankedMoves=(board:Stone[],side:Side,search:Search)=>search.position?.rank(side)??rank(board,side);
function play(board:Stone[],index:number,side:Side,search:Search){if(search.position)search.position.play(index,side);else board[index]=side;}
function unplay(board:Stone[],index:number,search:Search){if(search.position)search.position.undo(index);else board[index]=0;}
function expired(search: Search) {
  search.nodes++;
  if (search.nodes > search.maxNodes || performance.now() >= search.deadline) search.stopped = true;
  return search.stopped;
}

/** 连续冲四搜索：每一步均核对对手能否先成五，不能把自己的进攻当作对手必定配合。 */
function forcing(board: Stone[], side: Side, depth: number, search: Search): number[] | null {
  if (depth < 1 || expired(search)) return null;
  const ranked = rankedMoves(board, side,search), win = ranked.find(move => move.own.five);
  if (win) return [win.index];
  if (depth < 3) return null;
  const enemyWins = ranked.filter(move => move.enemy.five);
  if (enemyWins.length >= 2) return null;
  for (const move of ranked) {
    if (!move.own.wins.length || (enemyWins.length && move.index !== enemyWins[0]!.index)) continue;
    play(board,move.index,side,search);
    if (move.own.wins.length >= 2) {
      // 双冲四/活四：对手最多堵一个点，另一点即成五。
      const [block, finish] = move.own.wins;
      unplay(board,move.index,search);
      return [move.index, block!, finish!];
    }
    const response = move.own.wins[0]!;
    play(board,response,other(side),search);
    const continuation = forcing(board, side, depth - 2, search);
    unplay(board,response,search);unplay(board,move.index,search);
    if (continuation) return [move.index, response, ...continuation];
    if (search.stopped) break;
  }
  return null;
}

export function solveGomokuForcing(board: readonly Stone[], side: Side, options: ExpertOptions = {}) {
  const copy = [...board];
  const search: Search = { deadline: performance.now() + (options.timeMs ?? 300), nodes: 0, maxNodes: options.maxNodes ?? 8_000, stopped: false };
  if(options.incremental)search.position=new IncrementalPosition(copy);
  const line = forcing(copy, side, options.forcingDepth ?? 11, search);
  return { line, nodes: search.nodes, complete: !search.stopped };
}

// 连续威胁证明包含“活三”先手。没有冲四时枚举对手全部合法回应，
// 必须每条回应都能续杀才报告必胜，不能假设电脑会按参考棋谱配合。
function threatProof(board: Stone[], side: Side, depth: number, search: Search): number[] | null {
  if(depth<1||expired(search))return null;
  const moves=rankedMoves(board,side,search),win=moves.find(move=>move.own.five);
  if(win)return [win.index];
  if(depth<3)return null;
  const danger=moves.filter(move=>move.enemy.five);
  if(danger.length>1)return null;
  const attacks=moves.filter(move=>(move.own.fours||move.own.threes)&&(!danger.length||move.index===danger[0]!.index));
  for(const move of attacks){
    play(board,move.index,side,search);
    if(move.own.wins.length>1){unplay(board,move.index,search);return [move.index,move.own.wins[0]!,move.own.wins[1]!];}
    const replies=move.own.wins.length ? move.own.wins : rankedMoves(board,other(side),search).map(reply=>reply.index);
    // 邻近之外的空点也必须验证，保证安静先手的证明覆盖全棋盘。
    if(!move.own.wins.length)for(let i=0;i<225;i++)if(board[i]===0&&!replies.includes(i))replies.push(i);
    let proven=true,variation:number[]=[];
    for(const reply of replies){
      play(board,reply,other(side),search);
      // 对手落子若形成五连，进攻方已输，不能继续计算自己的杀法。
      const counter=gomokuThreat(board,reply,other(side)).five;
      const next=counter?null:threatProof(board,side,depth-2,search);
      unplay(board,reply,search);
      if(!next){proven=false;break;}
      if(!variation.length)variation=[move.index,reply,...next];
    }
    unplay(board,move.index,search);
    if(proven&&variation.length)return variation;
    if(search.stopped)break;
  }
  return null;
}

export function solveGomokuThreats(input: readonly Stone[], side: Side, options: ExpertOptions={}) {
  const board=[...input],search:Search={deadline:performance.now()+(options.timeMs??2500),maxNodes:options.maxNodes??30000,nodes:0,stopped:false};
  if(options.incremental)search.position=new IncrementalPosition(board);
  const line=threatProof(board,side,options.forcingDepth??13,search);
  return {line,nodes:search.nodes,complete:!search.stopped};
}

/** 最高档用更长的杀棋证明和更深搜索校验联网建议，计算始终在独立线程。 */
export function searchGomokuDeepSeek(input: readonly Stone[], side: Side, timeMs=8000, preferred?:number) {
  if(input.length!==225||input.some(stone=>![0,1,2].includes(stone)))return searchGomokuExpert(input,side);
  const start=performance.now();
  const direct=solveGomokuForcing(input,side,{timeMs:timeMs*.08,forcingDepth:19,maxNodes:12000,incremental:true});
  if(direct.line)return {index:direct.line[0]!,depth:direct.line.length,nodes:direct.nodes,line:direct.line};
  const threat=solveGomokuThreats(input,side,{timeMs:timeMs*.12,forcingDepth:13,maxNodes:30000,incremental:true});
  if(threat.line)return {index:threat.line[0]!,depth:threat.line.length,nodes:direct.nodes+threat.nodes,line:threat.line};
  // 先排除会让对方形成已证明连杀的着法，避免只顾自己的进攻或静态分数。
  const opponent=other(side),board=[...input];
  const enemyFour=solveGomokuForcing(board,opponent,{timeMs:timeMs*.04,forcingDepth:19,incremental:true});
  const enemy=enemyFour.line?enemyFour:solveGomokuThreats(board,opponent,{timeMs:timeMs*.06,forcingDepth:11,incremental:true});
  let rootCandidates:number[]|undefined;
  if(enemy.line&&enemy.line.length>1){
    const candidates=[...new Set([...rank(board,side).slice(0,24).map(move=>move.index),...enemy.line.filter(index=>!board[index])])];
    const unsafe=new Set<number>(),until=Math.min(start+timeMs*.5,performance.now()+timeMs*.25);
    for(const index of candidates){
      if(performance.now()>=until)break;
      board[index]=side;
      const four=solveGomokuForcing(board,opponent,{timeMs:Math.min(50,timeMs*.012),forcingDepth:19,incremental:true});
      const reply=four.line?four:solveGomokuThreats(board,opponent,{timeMs:Math.min(70,timeMs*.015),forcingDepth:11,incremental:true});
      board[index]=0;
      if(reply.line)unsafe.add(index);
    }
    const remaining=candidates.filter(index=>!unsafe.has(index));
    if(remaining.length)rootCandidates=remaining;
  }
  return searchGomokuExpert(input,side,{timeMs:Math.max(25,timeMs-(performance.now()-start)),maxDepth:14,forcingDepth:19,maxNodes:800000,preferred,incremental:true,rootCandidates});
}

/** 迭代加深的双方搜索；只采用完整搜索完的一层，超时保留上一层结果。 */
export function searchGomokuExpert(input: readonly Stone[], side: Side, options: ExpertOptions = {}) {
  const board = [...input], start = performance.now();
  const search: Search = { deadline: start + (options.timeMs ?? 1_800), maxNodes: options.maxNodes ?? 80_000, nodes: 0, stopped: false };
  let depthReached = 0;
  if (board.length !== 225 || board.some(stone => ![0, 1, 2].includes(stone))) return { index: null, depth: 0, nodes: 0, line: [] as number[] };
  if(options.incremental)search.position=new IncrementalPosition(board);
  const root = rankedMoves(board, side,search);
  if (!root.length) return { index: null, depth: 0, nodes: 0, line: [] as number[] };
  let best = root[0]!.index, principal: number[] = [best];
  const immediate = root.find(move => move.own.five);
  if (immediate) return { index: immediate.index, depth: 1, nodes: 1, line: [immediate.index] };
  const forcedBlock = root.filter(move => move.enemy.five);
  if (forcedBlock.length === 1) return { index: forcedBlock[0]!.index, depth: 1, nodes: 1, line: [forcedBlock[0]!.index] };
  const allowedRoot=options.rootCandidates?root.filter(move=>options.rootCandidates!.includes(move.index)):root;
  const rootSearch=allowedRoot.length?allowedRoot:root;
  best=rootSearch[0]!.index;principal=[best];
  // 给杀棋证明一个独立的小预算，剩余时间用于攻防搜索，防止无杀局面耗尽思考时间。
  const proof: Search = { ...search, deadline: start + (options.timeMs ?? 1_800) * .22, maxNodes: Math.min(search.maxNodes, 2_000) };
  const kill = forcing(board, side, options.forcingDepth ?? 11, proof);
  search.nodes += proof.nodes;
  if (kill) return { index: kill[0]!, depth: kill.length, nodes: search.nodes, line: kill };

  const transpositions = new Map<string, { depth: number; value: number; flag: 'exact' | 'lower' | 'upper'; move: number }>();
  function negamax(player: Side, depth: number, alpha: number, beta: number, ply: number): number {
    if (expired(search)) return 0;
    const key = player + ':' + board.join(''), cached = transpositions.get(key), originalAlpha = alpha;
    if (cached && cached.depth >= depth) {
      if (cached.flag === 'exact') return cached.value;
      if (cached.flag === 'lower') alpha = Math.max(alpha, cached.value); else beta = Math.min(beta, cached.value);
      if (alpha >= beta) return cached.value;
    }
    const moves = rankedMoves(board, player,search);
    if (!moves.length) return 0;
    const ownWin = moves.find(move => move.own.five);
    if (ownWin) return MATE - ply;
    const threats = moves.filter(move => move.enemy.five);
    if (threats.length >= 2) return -MATE + ply + 1;
    if (depth <= 0 && !threats.length) {
      if(moves.some(move=>move.own.fours)){
        const tactical:Search={deadline:Math.min(search.deadline,performance.now()+4),maxNodes:120,nodes:0,stopped:false,position:search.position};
        const continuation=forcing(board,player,7,tactical);search.nodes+=tactical.nodes;
        if(continuation)return MATE-ply-continuation.length;
      }
      const ours = moves.map(move => move.own.score).sort((a, b) => b - a), theirs = moves.map(move => move.enemy.score).sort((a, b) => b - a);
      return ours[0]! * 1.12 - theirs[0]! + (ours.slice(1, 4).reduce((a, b) => a + b, 0) - theirs.slice(1, 4).reduce((a, b) => a + b, 0)) * .12;
    }
    // 强制挡四允许延伸，不在必须回应的半截局面停下；仍由总时间和节点预算兜底。
    const chosen = threats.length ? threats : moves.slice(0, ply <= 1 ? 14 : depth >= 3 ? 10 : 8);
    if (cached) chosen.sort((a, b) => Number(b.index === cached.move) - Number(a.index === cached.move));
    let value = -Infinity, moveIndex = chosen[0]!.index;
    for (const move of chosen) {
      play(board,move.index,player,search);
      const next = -negamax(other(player), depth - 1, -beta, -alpha, ply + 1);
      unplay(board,move.index,search);
      if (search.stopped) return 0;
      if (next > value) { value = next; moveIndex = move.index; }
      alpha = Math.max(alpha, value);
      if (alpha >= beta) break;
    }
    transpositions.set(key, { depth, value, move: moveIndex, flag: value <= originalAlpha ? 'upper' : value >= beta ? 'lower' : 'exact' });
    return value;
  }
  for (let depth = 2; depth <= (options.maxDepth ?? 8); depth++) {
    let value = -Infinity, candidate = best;
    // 联网建议只改变强候选的搜索顺序；后续找到更高评价时仍会替换，不能跳过防守。
    const priority=(index:number)=>Number(index===options.preferred)*2+Number(index===best);
    const ordered = [...rootSearch.slice(0, 18)].sort((a, b) => priority(b.index)-priority(a.index));
    for (const move of ordered) {
      play(board,move.index,side,search);
      const score = -negamax(other(side), depth - 1, -Infinity, -value, 1);
      unplay(board,move.index,search);
      if (search.stopped) break;
      if (score > value) { value = score; candidate = move.index; }
    }
    if (search.stopped) break;
    best = candidate; depthReached = depth; principal = [best];
    if (Math.abs(value) >= MATE - 225) break;
  }
  return { index: best, depth: depthReached, nodes: search.nodes, line: principal };
}
