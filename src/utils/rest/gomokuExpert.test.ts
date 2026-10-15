// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createGomokuPuzzle, GOMOKU_PUZZLES, gomokuPuzzleContinuation, gomokuPuzzleTurns } from './gomokuCampaign';
import { GOMOKU_POSITIONS } from './gomokuPuzzles';
import { chooseGomokuMove, isGomokuState, placeGomoku, winningLine, type Stone } from './gomoku';
import { gomokuThreat, searchGomokuDeepSeek, searchGomokuExpert, solveGomokuForcing, solveGomokuThreats } from './gomokuExpert';

describe('DeepSeek最高档攻防校验',()=>{
  it('能证明包含安静先手的连续威胁，涵盖八题的全部合法防守',()=>{
    for(let level=0;level<30;level++)if(GOMOKU_POSITIONS[level]!.quiet){
      const {run}=createGomokuPuzzle(level),before=[...run.board];
      const result=solveGomokuThreats(run.board,run.human!,{timeMs:1500});
      expect(result.complete).toBe(true);expect(result.line,`第${level+1}题`).not.toBeNull();
      expect(run.board).toEqual(before);
      for(const index of result.line!)expect(placeGomoku(run,index,(run.moves.length%2+1) as 1|2)).toBe(true);
      expect(run.status).toBe('won');
    }
  },20000);
  it('联网给出弱建议也不能漏挡，自己的成五优先',()=>{
    const board=Array<Stone>(225).fill(0);for(const cell of [105,106,108,109])board[cell]=1;
    const before=[...board];expect(searchGomokuDeepSeek(board,2,300,0).index).toBe(107);expect(board).toEqual(before);
    for(const cell of [30,31,32,33])board[cell]=2;
    expect(searchGomokuDeepSeek(board,2,300,0).index).toBe(34);
  });
  it('增量棋形缓存撤子后与逐点计算一致，不污染原棋盘',()=>{
    for(const moves of [[112,128,97,129,144,110],[112,96,113,98,127,81]]){
      const board=Array<Stone>(225).fill(0);moves.forEach((cell,i)=>board[cell]=(i%2+1) as 1|2);
      const before=[...board],options={timeMs:5000,maxDepth:2,maxNodes:50000};
      const standard=searchGomokuExpert(board,1,options);
      const incremental=searchGomokuExpert(board,1,{...options,incremental:true});
      expect(incremental.depth).toBe(standard.depth);expect(incremental.index).toBe(standard.index);
      expect(board).toEqual(before);
    }
  },20000);
});

describe('高手战术搜索', () => {
  it('识别跳四中间的唯一成五点，不修改原棋盘；必胜优先于防守', () => {
    const board=Array<Stone>(225).fill(0);for(const cell of [105,106,108,109])board[cell]=1;
    const before=[...board];expect(searchGomokuExpert(board,2).index).toBe(107);expect(board).toEqual(before);
    for(const cell of [30,31,32,33])board[cell]=2;expect(searchGomokuExpert(board,2).index).toBe(34);
  });
  it('主动找到多回合连续杀棋，完整复现可取胜，而非只找当前最高分', () => {
    for(const level of [0,24,27,28,29]){
      const {run}=createGomokuPuzzle(level),original=[...run.board];
      const proof=solveGomokuForcing(run.board,run.human!,{timeMs:1000,forcingDepth:11});
      expect(proof.line,`第${level+1}题`).not.toBeNull();expect(run.board).toEqual(original);
      expect(searchGomokuExpert(run.board,run.human!,{timeMs:1000}).index).toBe(proof.line![0]);
      for(const cell of proof.line!)expect(placeGomoku(run,cell,(run.moves.length%2+1) as 1|2)).toBe(true);
      expect(run.status).toBe('won');
    }
  });
  it('没有杀棋时也完成多层攻防搜索；预算用尽返回合法落点', () => {
    const board=Array<Stone>(225).fill(0);[112,128,97,129,144,110].forEach((cell,i)=>board[cell]=(i%2+1) as 1|2);
    const result=searchGomokuExpert(board,1,{timeMs:1500,maxDepth:3,maxNodes:15000});
    expect(result.depth).toBeGreaterThanOrEqual(2);expect(board[result.index!]).toBe(0);
    const bounded=searchGomokuExpert(board,2,{timeMs:1,maxNodes:1});expect(board[bounded.index!]).toBe(0);
    expect(searchGomokuExpert(Array(225).fill(1),2).index).toBeNull();
  });
  it('跳活三与封口三区别明确，棋盘边缘不能凭空产生开放端', () => {
    const board=Array<Stone>(225).fill(0);board[107]=1;board[109]=1;
    expect(gomokuThreat(board,110,1).threes).toBeGreaterThan(0);
    const edge=Array<Stone>(225).fill(0);edge[0]=1;edge[1]=1;
    expect(gomokuThreat(edge,2,1).threes).toBe(0);
  });
});

describe('独立设计的30个实战残局', () => {
  it('每题合法、没有现成的一步成五，3—6手内实际连五才结束', () => {
    const unique=new Set<string>();expect(GOMOKU_PUZZLES).toHaveLength(30);
    for(let level=0;level<30;level++){
      const {run,solution}=createGomokuPuzzle(level);unique.add(run.board.join(''));expect(isGomokuState(run)).toBe(true);
      expect(run.board.some((v,i)=>!v&&gomokuThreat(run.board,i,run.human!).five),`第${level+1}题不应有一手收官`).toBe(false);
      expect(gomokuPuzzleTurns(level,run)).toBe(0);
      for(let step=0;step<solution.length;step++){
        expect(gomokuPuzzleContinuation(level,run)).toBe(solution[step]);
        expect(placeGomoku(run,solution[step]!, (run.moves.length%2+1) as 1|2)).toBe(true);
        if(step<solution.length-1)expect(run.status).toBe('playing');
      }
      expect(run.status).toBe('won');expect(isGomokuState(run)).toBe(true);expect(gomokuPuzzleTurns(level,run)).toBe(GOMOKU_PUZZLES[level]!.turns);
    }
    expect(unique.size).toBe(30);expect(new Set(GOMOKU_PUZZLES.map(p=>p.human)).size).toBe(2);
  });
  it('八道双三题逐一验证每一种合法防守，不能依赖电脑配合固定答案', () => {
    for(let level=0;level<30;level++)if(GOMOKU_POSITIONS[level]!.quiet){
      const {run,answer}=createGomokuPuzzle(level),side=run.human!,enemy=side===1?2:1;
      run.board[answer]=side;
      for(let reply=0;reply<225;reply++)if(!run.board[reply]){
        run.board[reply]=enemy;expect(winningLine(run.board,reply)).toHaveLength(0);
        const proof=solveGomokuForcing(run.board,side,{timeMs:300,forcingDepth:7});
        expect(proof.line,`题${level+1},防守${reply}`).not.toBeNull();run.board[reply]=0;
      }
    }
  },30000);
  it('电脑不同应对时仍可自由落子，参考提示不会误用旧局面', () => {
    const {run}=createGomokuPuzzle(0);const cell=run.board.findIndex(v=>v===0);placeGomoku(run,cell,run.human!);
    expect(gomokuPuzzleContinuation(0,run)).toBeNull();
    expect(chooseGomokuMove(run.board,'hard',()=>.5,run.human===1?2:1)).not.toBeNull();
  });
});
