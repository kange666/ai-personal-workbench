import { describe, expect, it } from 'vitest';
import { createDefense, createDefenseMeta, defenseOffers, DEFENSE_CARDS, defenseStats, pickDefenseCard, settleDefense, startDefense, startDefenseWave, stepDefense, upgradeDefense, validDefense } from './defense';
import { createCircuit, circuitStatus, turnCircuit, hintCircuit, undoCircuit, validCircuit } from './wiringCampaign';
import { createTowerAdventure, dropTowerAdventure, stepTowerAdventure, towerTool, validTowerAdventure, TOWER_LEVELS } from './towerCampaign';
import { startTower } from './tower';
import { breakoutLayout, createBreakoutAdventure, giveBreakoutPower, startBreakoutAdventure, stepBreakoutAdventure, validBreakoutAdventure, nextBreakoutAdventure } from './breakoutCampaign';
import { createGomokuPuzzle, correctGomokuPuzzle, GOMOKU_PUZZLES, createGomokuSession, migrateGomoku, validGomokuSession } from './gomokuCampaign';
import { chooseGomokuMove, createGomoku, isGomokuState, placeGomoku, winningLine } from './gomoku';
import { seeded } from './progression';
import { migrateSokoban, createSokobanState, SOKOBAN_LEVELS, sokobanDeadlock, getSokobanBoard, solveSokoban, moveSokobanSnapshot, isSokobanSolved } from './sokoban';

describe('电路六十关',()=>{
  it('每关都能通过合法旋转接通，双色桥接不短路，读档与撤销有效',()=>{
    const maps=new Set<string>();
    for(let level=0;level<60;level++){
      const s=createCircuit(level);maps.add(JSON.stringify(s.solution));expect(validCircuit(s),`初始 ${level}`).toBe(true);expect(s.won).toBe(false);
      const before=JSON.stringify(s.tiles);hintCircuit(s);undoCircuit(s);expect(JSON.stringify(s.tiles)).toBe(before);
      for(let i=0;i<s.tiles.length;i++)for(let r=0;s.tiles[i]!==s.solution[i]&&r<4;r++)turnCircuit(s,i);
      expect(circuitStatus(s),`关 ${level+1}`).toMatchObject({won:true,mixed:false,leaks:0});expect(validCircuit(JSON.parse(JSON.stringify(s)))).toBe(true);
    }
    expect(maps.size).toBe(60);
  });
  it('固定格不可转动，篡改的撤销形状不能进入存档',()=>{
    const s=createCircuit(48),i=s.locked.findIndex(Boolean),before=s.tiles[i];turnCircuit(s,i);expect(s.tiles[i]).toBe(before);
    s.history=[Array(s.tiles.length).fill(15)];expect(validCircuit(s)).toBe(false);
  });
});


describe('孤塔守夜平衡与规则',()=>{
  const simulate=(seed:number,family:string)=>{
    const s=createDefense(seed),meta=createDefenseMeta();startDefense(s,meta);
    const purchases=['attack','rate','attack','range','health','rate','regen','income'] as const;let buy=0;
    for(let n=0;n<220000&&!['won','lost'].includes(s.phase);n++){
      if(s.pendingUpgrades>0&&s.phase==='playing')defenseOffers(s);
      if(s.phase==='choice'){
        const preferred=s.offers.find(id=>DEFENSE_CARDS.find(c=>c.id===id)?.family===family)||s.offers.find(id=>['power','speed','multi','blast','chain','health'].includes(id))||s.offers[0]!;
        pickDefenseCard(s,preferred);
      }
      for(let tries=0;tries<4;tries++){const id=purchases[buy%purchases.length]!;if(upgradeDefense(s,id))buy++;else break;}
      stepDefense(s,.1);
      if(n%500===0)expect(validDefense(JSON.parse(JSON.stringify(s))),`wave ${s.wave}`).toBe(true);
    }
    return s;
  };
  it('36 张强化无重复，三选一不会出现缺少基础能力的空效果卡',()=>{
    expect(new Set(DEFENSE_CARDS.map(c=>c.id)).size).toBe(36);
    const s=createDefense(5);s.wave=5;s.phase='playing';s.pendingUpgrades=1;defenseOffers(s);expect(s.offers.some(id=>['radius','links','inferno','freeze','storm'].includes(id))).toBe(false);
    const before=JSON.stringify(s);expect(pickDefenseCard(s,'invalid')).toBe(false);expect(JSON.stringify(s)).toBe(before);
    const clone=JSON.parse(before);expect(pickDefenseCard(s,s.offers[0]!)).toBe(true);pickDefenseCard(clone,clone.offers[0]);expect(clone).toEqual(s);
  });
  it('按推荐升级路线可通关，四类组合都可成长，电脑敌人仍形成压力',()=>{
    const results=['速射','爆破','堡垒','成长'].map((family,i)=>simulate(100+i,family));
    console.info('塔防平衡结果',results.map(s=>({wave:s.wave,hp:Math.round(s.hp),phase:s.phase,kills:s.kills})));
    for(const s of results){expect(s.phase).toBe('won');expect(s.wave).toBe(30);expect(s.kills).toBeGreaterThan(400);}
  },30000);
  it('不升级也能度过前三波，后期需要选择强化与升级',()=>{
    const s=createDefense(11);startDefense(s,createDefenseMeta());
    for(let n=0;n<40000&&s.wave<4&&s.phase!=='lost';n++){if(s.pendingUpgrades&&s.phase==='playing')defenseOffers(s);if(s.phase==='choice')pickDefenseCard(s,s.offers[0]!);stepDefense(s,.1);}
    expect(s.wave).toBeGreaterThanOrEqual(4);expect(s.hp).toBeGreaterThan(0);
  });
  it('选卡冻结计时、存档复现战况，奖励只结算一次；闯关全卡6级后仍可强化',()=>{
    const s=createDefense(71);startDefense(s,createDefenseMeta());for(let i=0;i<200;i++)stepDefense(s,.05);
    const copy=JSON.parse(JSON.stringify(s));for(let i=0;i<200;i++){stepDefense(s,.05);stepDefense(copy,.05);}expect(copy).toEqual(s);
    s.pendingUpgrades=1;defenseOffers(s);const before=JSON.stringify(s);stepDefense(s,.1);expect(JSON.stringify(s)).toBe(before);
    const meta=createDefenseMeta();s.phase='won';s.wave=30;settleDefense(s,meta);const shards=meta.shards;settleDefense(s,meta);expect(meta.shards).toBe(shards);s.mode='endless';s.wave=31;s.phase='lost';settleDefense(s,meta);expect(meta.shards).toBe(shards);
    s.mode='campaign';s.phase='playing';delete s.choiceResume;s.pendingUpgrades=1;s.cards=Object.fromEntries(DEFENSE_CARDS.map(c=>[c.id,6]));defenseOffers(s);expect(s.phase).toBe('choice');expect(s.wave).toBe(31);expect(s.pendingUpgrades).toBe(1);expect(validDefense(s)).toBe(true);
  });
  it('四个流派的核心升级确实改变能力',()=>{
    const s=createDefense(1),base=defenseStats(s);s.cards={power:2,speed:2,multi:2,health:2,armor:2,regen:2};const stats=defenseStats(s);
    expect(stats.damage).toBeGreaterThan(base.damage);expect(stats.rate).toBeGreaterThan(base.rate);expect(stats.multi).toBe(3);expect(stats.maxHP).toBeGreaterThan(base.maxHP);expect(stats.armor).toBeGreaterThan(0);expect(stats.regen).toBeGreaterThan(base.regen);
  });
  it('自动推进下一波与主动选卡都只记录已经守住的波数',()=>{
    const s=createDefense(81),meta=createDefenseMeta();s.phase='playing';s.wave=1;settleDefense(s,meta);expect(meta.bestWave).toBe(0);
    s.wave=2;s.pendingUpgrades=1;settleDefense(s,meta);expect(meta.bestWave).toBe(1);
    defenseOffers(s);settleDefense(s,meta);expect(meta.bestWave).toBe(1);
  });
});

describe('叠高塔四十关',()=>{
  it('每关的目标均能实际达成并恢复存档，速度有限且不会越界',()=>{
    for(let level=0;level<40;level++){
      const s=createTowerAdventure(level);startTower(s.run);
      for(let i=0;i<TOWER_LEVELS[level].height;i++){stepTowerAdventure(s,.05);s.run.movingX=s.run.blocks.at(-1)!.x;dropTowerAdventure(s);expect(validTowerAdventure(s)).toBe(true);}
      expect(s.won).toBe(true);expect(validTowerAdventure(JSON.parse(JSON.stringify(s)))).toBe(true);
    }
  });
  it('修补不超出下层，失误可回退一次，慢速次数有限',()=>{
    const s=createTowerAdventure();startTower(s.run);s.run.movingX=240;dropTowerAdventure(s);expect(towerTool(s,'repair')).toBe(true);expect(validTowerAdventure(s)).toBe(true);expect(towerTool(s,'repair')).toBe(false);
    s.run.movingX=0;dropTowerAdventure(s);expect(s.run.phase).toBe('over');expect(towerTool(s,'undo')).toBe(true);expect(s.run.phase).toBe('moving');expect(towerTool(s,'undo')).toBe(false);
    expect(towerTool(s,'slow')).toBe(true);expect(towerTool(s,'slow')).toBe(true);expect(towerTool(s,'slow')).toBe(false);expect(validTowerAdventure(s)).toBe(true);
  });
  it('万层无尽局的回退只保存一层差异，仍能完整读档并控制体积',()=>{
    const s=createTowerAdventure(0,'endless');s.run.blocks=Array.from({length:9999},()=>({x:150,width:180}));s.run.score=9998*35-50;s.run.perfectStreak=9998;s.run.lastPerfect=true;s.run.phase='moving';s.run.movingX=150;s.perfects=9998;s.maxStreak=9998;
    dropTowerAdventure(s);expect(s.run.phase).toBe('complete');expect(validTowerAdventure(s)).toBe(true);expect(JSON.stringify(s).length).toBeLessThan(1_000_000);expect(s.previous!.length).toBe(9999);
  });
});

describe('弹球四十关与六种补给',()=>{
  it('四十张布局互不相同、五场首领战，开关和补给位置有效',()=>{
    expect(new Set(Array.from({length:40},(_,i)=>JSON.stringify(breakoutLayout(i)))).size).toBe(40);
    expect(Array.from({length:40},(_,i)=>breakoutLayout(i)).filter(a=>a.some(b=>b.kind==='boss'))).toHaveLength(5);
    for(let i=0;i<40;i++)expect(validBreakoutAdventure(createBreakoutAdventure(i))).toBe(true);
  });
  it('多球尚存时不扣生命，全丢后只扣一次，护底救球',()=>{
    const s=createBreakoutAdventure();startBreakoutAdventure(s);giveBreakoutPower(s,'multi');expect(s.balls).toHaveLength(3);s.effects.shield=0;s.balls[0]!.y=329;s.balls[0]!.vy=300;stepBreakoutAdventure(s,.05);expect(s.balls).toHaveLength(2);expect(s.lives).toBe(4);
    s.balls.forEach(b=>{b.y=329;b.vy=300;});stepBreakoutAdventure(s,.05);expect(s.phase).toBe('ready');expect(s.lives).toBe(3);stepBreakoutAdventure(s,.05);expect(s.lives).toBe(3);
    startBreakoutAdventure(s);giveBreakoutPower(s,'shield');s.balls[0]={x:20,y:312,vx:0,vy:200};stepBreakoutAdventure(s,.05);expect(s.balls[0]!.vy).toBeLessThan(0);expect(s.lives).toBe(3);
  });
  it('高速碰撞不穿砖，激光和穿透有效，计时和轨迹读档一致',()=>{
    const s=createBreakoutAdventure(0,'campaign',42);startBreakoutAdventure(s);s.balls=[{x:37,y:120,vx:0,vy:-1000}];stepBreakoutAdventure(s,.05);expect(s.bricks.some(b=>b.hp===0)).toBe(true);
    for(const kind of ['wide','laser','slow','pierce'] as const)giveBreakoutPower(s,kind);
    const copy=JSON.parse(JSON.stringify(s));for(let i=0;i<300;i++){stepBreakoutAdventure(s,.05);stepBreakoutAdventure(copy,.05);}expect(copy).toEqual(s);expect(validBreakoutAdventure(s)).toBe(true);
    s.phase='won';s.lives=2;s.score=500;const next=nextBreakoutAdventure(s);expect(next.level).toBe(1);expect(next.lives).toBe(3);expect(next.score).toBe(500);
  });
  it('四十关通过连续物理回合可清空，锁砖总有可用开关',()=>{
    for(let level=0;level<40;level++){
      const s=createBreakoutAdventure(level,'campaign',level+80);startBreakoutAdventure(s);
      if(s.bricks.some(b=>b.kind==='metal'))expect(s.bricks.some(b=>b.kind==='switch')).toBe(true);
      for(let frame=0;frame<50000&&s.phase!=='won'&&s.phase!=='lost';frame++){
        if(s.phase==='ready')startBreakoutAdventure(s);
        const ball=s.balls.find(b=>b.vy>0&&b.y>200);
        if(ball){
          const t=Math.max(0,(281-ball.y)/ball.vy),raw=ball.x+ball.vx*t-6,fold=((raw%936)+936)%936,land=6+(fold>468?936-fold:fold);
          const remaining=s.bricks.filter(b=>b.hp>0&&!['boss','metal'].includes(b.kind)),target=remaining.length?remaining[Math.floor(frame/200)%remaining.length]!:s.bricks.find(b=>b.kind==='boss'&&b.hp>0);
          const angle=target?Math.atan2(target.x+target.w/2-land,281-target.y):.2;
          const half=(s.effects.wide>0?128:94)/2,offset=Math.max(-half*.8,Math.min(half*.8,angle/1.05*half));s.paddleX=land-offset;
        }
        stepBreakoutAdventure(s,.05);
      }
      expect(s.phase,`第 ${level+1} 关余砖 ${s.bricks.filter(b=>b.hp>0&&b.kind!=='metal').map(b=>b.id+':'+b.hp)}`).toBe('won');
      expect(validBreakoutAdventure(s)).toBe(true);
    }
  },30000);
});

describe('五子棋降低默认强度与三十道练习',()=>{
  it('默认入门、旧棋局和最高分可迁移，玩家可执白',()=>{
    expect(createGomokuSession().difficulty).toBe('relaxed');const s=createGomoku();placeGomoku(s,112,1);const migrated=migrateGomoku(s)!;expect(migrated.run).toEqual(s);expect(validGomokuSession(migrated)).toBe(true);
    const white=createGomoku(2);expect(placeGomoku(white,112,1)).toBe(true);expect(placeGomoku(white,113,2)).toBe(true);expect(isGomokuState(white)).toBe(true);
  });
  it('每道残局棋局合法，必须完成整段杀法才能连成五子',()=>{
    const unique=new Set<string>();expect(GOMOKU_PUZZLES).toHaveLength(30);
    for(let i=0;i<30;i++){const {run,answer,solution}=createGomokuPuzzle(i);unique.add(run.board.join(''));expect(isGomokuState(run),`第 ${i+1} 题`).toBe(true);expect(run.status).toBe('playing');expect(correctGomokuPuzzle(i,run,answer)).toBe(true);
      expect(solution.length).toBeGreaterThanOrEqual(5);for(const cell of solution)expect(placeGomoku(run,cell,(run.moves.length%2+1) as 1|2)).toBe(true);expect(run.status).toBe('won');expect(isGomokuState(run)).toBe(true);
    }expect(unique.size).toBe(30);
  });
  it('入门会留下明显防守机会，困难会封堵唯一成五口',()=>{
    const run=createGomoku(),answer=109;for(const cell of [105,106,107,108])run.board[cell]=1;const rng=seeded(14);let misses=0;
    for(let i=0;i<100;i++)if(chooseGomokuMove([...run.board],'relaxed',rng)!==answer)misses++;
    expect(misses).toBeGreaterThan(20);expect(chooseGomokuMove(run.board,'hard',rng)).toBe(answer);
  });
});

describe('推箱子内容与迁移',()=>{
  it('八十关地图不同，老六关存档补齐星级容量后保留棋盘',()=>{
    expect(new Set(SOKOBAN_LEVELS.map(l=>l.map.join('\n'))).size).toBe(80);const old=createSokobanState();old.awards=old.awards.slice(0,6);const next=migrateSokoban(old)!;expect(next.awards).toHaveLength(80);expect(next.player).toBe(old.player);expect(next.boxes).toEqual(old.boxes);
    const b=getSokobanBoard(0);expect(sokobanDeadlock(b,[8])).toBe(true);expect(sokobanDeadlock(b,b.goals)).toBe(false);expect(solveSokoban(b,{player:b.player,boxes:b.boxes,moves:0,pushes:0})).toEqual(['left','left']);
  });
  it('原六关的三星参考步数也有完整合法路线，不会设成无法达到的二十步',()=>{
    const delta:Record<string,'up'|'right'|'down'|'left'>={U:'up',R:'right',D:'down',L:'left'};
    for(let level=0;level<6;level++){
      const board=getSokobanBoard(level);let s={player:board.player,boxes:board.boxes,moves:0,pushes:0};
      for(const letter of SOKOBAN_LEVELS[level].solution){const moved=moveSokobanSnapshot(board,s,delta[letter]);expect(moved).not.toBeNull();s=moved!;}
      expect(isSokobanSolved(board,s.boxes)).toBe(true);
    }
  });
});
