import { clamp, finite, integer, object, random } from './progression';
export type BreakoutPower = 'multi' | 'wide' | 'laser' | 'slow' | 'pierce' | 'shield';
export const BREAKOUT_POWER_NAMES: Record<BreakoutPower,string> = { multi:'多球',wide:'宽挡板',laser:'激光',slow:'慢速',pierce:'穿透',shield:'护底' };
export const BREAKOUT_LEVELS = Array.from({length:40},(_,i)=>({name:`${['星港启航','棱镜阵列','机关工厂','流动壁垒','星核远征'][Math.floor(i/8)]} ${i%8===7?'· 首领':i%8+1}`,chapter:`第 ${Math.floor(i/8)+1} 章`}));
export interface AdventureBall { x:number;y:number;vx:number;vy:number }
export interface AdventureBrick { id:number;x:number;y:number;w:number;h:number;hp:number;maxHP:number;kind:'normal'|'hard'|'bomb'|'switch'|'metal'|'moving'|'boss' }
export interface BreakoutAdventure {version:1;level:number;mode:'campaign'|'endless';seed:number;phase:'ready'|'playing'|'won'|'lost';lives:number;score:number;paddleX:number;balls:AdventureBall[];bricks:AdventureBrick[];items:{x:number;y:number;kind:BreakoutPower}[];effects:Record<BreakoutPower,number>;time:number;laserTime:number;broken:number;lostBalls:number;last:string}
export function breakoutLayout(level:number):AdventureBrick[] {
  const bricks:AdventureBrick[]=[], chapter=Math.floor(level/8)%5, pattern=level%8;
  for(let r=0;r<5;r++)for(let c=0;c<8;c++){
    const include=pattern===0?true:pattern===1?(r+c)%3!==0:pattern===2?Math.abs(c-3.5)+r<6:pattern===3?r===0||r===4||c%2===0:pattern===4?(r*2+c)%4!==0:pattern===5?r===2||c===r||c===7-r:pattern===6?(c+r)%2===0:r<3;
    if(!include)continue;
    let kind:AdventureBrick['kind']='normal';
    if(chapter>=1&&(c+r)%5===0)kind='hard';
    if(chapter>=1&&(c*2+r+level)%11===0)kind='bomb';
    if(chapter>=2&&r===0&&c===3)kind='switch';
    if(chapter>=2&&r===3&&c%3===1)kind='metal';
    if(chapter>=3&&r===1&&c%2===0)kind='moving';
    const hp=kind==='hard'?2+Math.floor(chapter/2):kind==='metal'?999:1;
    bricks.push({id:r*8+c,x:12+c*58,y:38+r*23,w:50,h:15,hp,maxHP:hp,kind});
  }
  if(bricks.some(b=>b.kind==='metal')&&!bricks.some(b=>b.kind==='switch')){const b=bricks.find(b=>b.kind==='normal')!;b.kind='switch';}
  // 首领放在护卫后方，避免无敌阶段挡住最后一块护卫，使前期对战更直观。
  if(pattern===7)bricks.push({id:100,x:180,y:10,w:120,h:20,hp:10+chapter*4,maxHP:10+chapter*4,kind:'boss'});
  return bricks;
}
export function createBreakoutAdventure(level=0,mode:'campaign'|'endless'='campaign',seed=Date.now()>>>0):BreakoutAdventure {
  return{version:1,level,mode,seed,phase:'ready',lives:4,score:0,paddleX:240,balls:[{x:240,y:278,vx:125,vy:-205}],bricks:breakoutLayout(level),items:[],effects:{multi:0,wide:0,laser:0,slow:0,pierce:0,shield:1},time:0,laserTime:0,broken:0,lostBalls:0,last:'移动挡板接住球；每三块砖都有补给。'};
}
export function breakoutPaddleWidth(s:BreakoutAdventure){return s.effects.wide>0?128:94;}
export function startBreakoutAdventure(s:BreakoutAdventure){if(s.phase==='ready')s.phase='playing';}
export function breakoutBrickX(s:BreakoutAdventure,b:AdventureBrick){return b.kind==='moving'?clamp(b.x+Math.sin(s.time*1.4+b.id)*7,6,474-b.w):b.x;}
export function giveBreakoutPower(s:BreakoutAdventure,kind:BreakoutPower){
  if(kind==='multi'){
    const source=s.balls[0];if(!source)return;
    for(const sign of [-1,1])if(s.balls.length<7)s.balls.push({...source,vx:sign*(Math.abs(source.vx)+45),vy:-Math.max(150,Math.abs(source.vy))});
  }else if(kind==='shield')s.effects.shield=Math.min(3,s.effects.shield+1);
  else s.effects[kind]=Math.max(s.effects[kind],kind==='wide'?22:kind==='slow'?18:14);
  s.last=`获得${BREAKOUT_POWER_NAMES[kind]}！`;
}
function damageBrick(s:BreakoutAdventure,b:AdventureBrick,damage=1){
  if(b.hp<=0||b.kind==='metal'||(b.kind==='boss'&&s.bricks.some(x=>x.hp>0&&!['boss','metal'].includes(x.kind))))return;
  b.hp-=damage;if(b.hp>0){s.score+=5;return;}
  b.hp=0;s.broken++;s.score+=b.kind==='boss'?500:25;
  if(s.broken%3===0){const types=Object.keys(BREAKOUT_POWER_NAMES) as BreakoutPower[];s.items.push({x:b.x+b.w/2,y:b.y,kind:types[(s.broken/3-1+Math.floor(random(s)*2))%types.length]!});}
  if(b.kind==='switch')for(const other of s.bricks)if(other.kind==='metal')other.hp=0;
  if(b.kind==='bomb')for(const near of s.bricks)if(near!==b&&near.hp>0&&Math.hypot(near.x-b.x,near.y-b.y)<80&&near.kind!=='bomb')damageBrick(s,near,2);
}
export function stepBreakoutAdventure(s:BreakoutAdventure,seconds:number){
  if(s.phase!=='playing'||!finite(seconds,.000001,.2))return;
  const dt=Math.min(seconds,.05);s.time+=dt;s.laserTime+=dt;
  for(const kind of ['wide','laser','slow','pierce'] as const)s.effects[kind]=Math.max(0,s.effects[kind]-dt);
  const half=breakoutPaddleWidth(s)/2;s.paddleX=clamp(s.paddleX,half,480-half);
  if(s.effects.laser>0&&s.laserTime>=.45){s.laserTime=0;for(const x of [s.paddleX-half+8,s.paddleX+half-8]){const b=s.bricks.filter(b=>b.hp>0&&x>=breakoutBrickX(s,b)&&x<=breakoutBrickX(s,b)+b.w).sort((a,b)=>b.y-a.y)[0];if(b)damageBrick(s,b);}}
  const active:AdventureBall[]=[];
  for(const ball of [...s.balls]){
    const speed=Math.hypot(ball.vx,ball.vy);if(speed>350){ball.vx*=350/speed;ball.vy*=350/speed;}
    const n=Math.max(1,Math.ceil(Math.hypot(ball.vx,ball.vy)*dt/2.5)), sub=dt/n*(s.effects.slow>0?.65:1);
    const hit=new Set<number>();
    for(let i=0;i<n;i++){
      const oldX=ball.x,oldY=ball.y;ball.x+=ball.vx*sub;ball.y+=ball.vy*sub;
      if(ball.x<6){ball.x=6;ball.vx=Math.abs(ball.vx);}if(ball.x>474){ball.x=474;ball.vx=-Math.abs(ball.vx);}if(ball.y<6){ball.y=6;ball.vy=Math.abs(ball.vy);}
      if(ball.vy>0&&oldY<=281&&ball.y>=281&&ball.x>=s.paddleX-half-5&&ball.x<=s.paddleX+half+5){
        const offset=clamp((ball.x-s.paddleX)/half,-.95,.95),velocity=Math.max(235,Math.min(335,Math.hypot(ball.vx,ball.vy)+1));
        ball.y=281;ball.vx=velocity*Math.sin(offset*1.05);ball.vy=-Math.max(135,velocity*Math.cos(offset*1.05));
      }
      for(const b of s.bricks){
        if(b.hp<=0||hit.has(b.id))continue;const bx=breakoutBrickX(s,b);
        if(ball.x+6<bx||ball.x-6>bx+b.w||ball.y+6<b.y||ball.y-6>b.y+b.h)continue;
        hit.add(b.id);damageBrick(s,b,s.effects.pierce>0?2:1);
        if(s.effects.pierce<=0||b.kind==='metal'||b.kind==='boss'){
          if(oldY+6<=b.y||oldY-6>=b.y+b.h){ball.vy=-ball.vy;ball.y=oldY;}else{ball.vx=-ball.vx;ball.x=oldX;}break;
        }
      }
      if(ball.y>=313&&s.effects.shield>0){s.effects.shield--;ball.y=309;ball.vy=-Math.abs(ball.vy);s.last='护底挡住了一次失球';}
      if(ball.y>330)break;
    }
    if(ball.y<=330)active.push(ball);
  }
  s.balls=active;
  for(const item of s.items){item.y+=85*dt;if(item.y>=282&&item.y<=300&&Math.abs(item.x-s.paddleX)<=half+8){giveBreakoutPower(s,item.kind);item.y=999;}}
  s.items=s.items.filter(item=>item.y<330).slice(-30);
  if(!s.bricks.some(b=>b.hp>0&&b.kind!=='metal')){s.phase='won';s.score+=s.lives*50;s.last='阵列清空，补给完毕！';return;}
  if(s.balls.length===0){s.lives--;s.lostBalls++;s.effects.wide=0;s.effects.laser=0;s.effects.pierce=0;
    if(s.lives<=0){s.phase='lost';s.last='试试宽挡板和护底补给，下次会更稳。';}
    else{s.phase='ready';s.balls=[{x:s.paddleX,y:278,vx:125,vy:-205}];s.last='点击继续发球，剩余进度保留。';}
  }
}
export function nextBreakoutAdventure(s:BreakoutAdventure):BreakoutAdventure {
  if(s.phase!=='won')return s;
  const next=createBreakoutAdventure(s.mode==='campaign'?Math.min(39,s.level+1):s.level+1,s.mode,s.seed);
  next.score=s.score;next.lives=Math.min(4,s.lives+1);return next;
}
export function validBreakoutAdventure(value:unknown):value is BreakoutAdventure {
  if(!object(value))return false;const s=value as unknown as BreakoutAdventure;
  if(s.version!==1||!integer(s.level,0,100000)||!['campaign','endless'].includes(s.mode)||(s.mode==='campaign'&&s.level>39)||!integer(s.seed,0,4294967295)||!['ready','playing','won','lost'].includes(s.phase)||!integer(s.lives,0,4)||!integer(s.score,0,1e12)||!finite(s.paddleX,0,480)||!finite(s.time,0,1e12)||!finite(s.laserTime,0,1e12)||!integer(s.broken)||!integer(s.lostBalls)||typeof s.last!=='string')return false;
  if(!Array.isArray(s.balls)||s.balls.length>7||!s.balls.every(b=>object(b)&&finite(b.x,-10,490)&&finite(b.y,-10,335)&&finite(b.vx,-500,500)&&finite(b.vy,-500,500))||!Array.isArray(s.bricks)||s.bricks.length>41)return false;
  const layout=breakoutLayout(s.level);if(s.bricks.length!==layout.length||!s.bricks.every((b,i)=>{const ref=layout[i];return object(b)&&b.id===ref.id&&b.x===ref.x&&b.y===ref.y&&b.w===ref.w&&b.h===ref.h&&b.kind===ref.kind&&b.maxHP===ref.maxHP&&integer(b.hp,0,b.maxHP);}))return false;
  return object(s.effects)&&(Object.keys(BREAKOUT_POWER_NAMES) as BreakoutPower[]).every(k=>finite(s.effects[k],0,k==='shield'?3:30))&&Array.isArray(s.items)&&s.items.length<=30&&s.items.every(i=>object(i)&&finite(i.x,0,480)&&finite(i.y,0,330)&&Object.hasOwn(BREAKOUT_POWER_NAMES,i.kind));
}
