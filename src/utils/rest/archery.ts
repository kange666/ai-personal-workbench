import { clamp, finite, integer, object, random } from "./progression";
export type ArrowKind="normal"|"heavy"|"blast"|"bounce";
export type ArcherSide="player"|"enemy";
export interface ArcherObstacle {x:number;y:number;width:number;height:number;kind:"stone"|"wood"|"moving"|"reflect";hp:number}
export interface ArcherShot {x:number;y:number;vx:number;vy:number;owner:ArcherSide;kind:ArrowKind;bounces:number;age:number;trail:{x:number;y:number}[]}
export interface ArcheryState {version:1;mode:'campaign'|'endless';level:number;seed:number;phase:"aim"|"flight"|"ai"|"won"|"lost";playerHP:number;enemyHP:number;enemyMax:number;wind:number;time:number;aiTime:number;turn:number;angle:number;power:number;kind:ArrowKind;ammo:Record<ArrowKind,number>;obstacles:ArcherObstacle[];shot:ArcherShot|null;last:string;hits:number;headshots:number;shots:number;score:number;assist:boolean}
export const ARCHERY_LEVELS=Array.from({length:30},(_,i)=>({name:["平地试射","越过矮墙","高台对射","木障破局","风向练习"][i%5]!+` ${Math.floor(i/5)+1}`,chapter:["初学弓手","风中对决","机关战地"][Math.floor(i/10)]!}));
export const ARROWS=[{id:"normal",name:"普通箭"},{id:"heavy",name:"重箭"},{id:"blast",name:"爆破箭"},{id:"bounce",name:"反弹箭"}] as const;
export function archerPosition(level:number,side:ArcherSide){return {x:side==="player"?60:580,y:side==="player"?260+level%3*6:250-Math.floor(level/10)*10+level%4*8};}
export function obstacleAt(obstacle:ArcherObstacle,time:number){return {...obstacle,y:obstacle.y+(obstacle.kind==="moving"?Math.sin(time*1.25)*28:0)};}
export function createArchery(level=0,seed=Date.now()>>>0):ArcheryState {
  level=clamp(Math.floor(level),0,29);
  const s:ArcheryState={version:1,mode:'campaign',level,seed,phase:"aim",playerHP:130,enemyHP:80+Math.floor(level/10)*10,enemyMax:80+Math.floor(level/10)*10,wind:0,time:0,aiTime:0,turn:0,angle:48,power:350,kind:"normal",ammo:{normal:999,heavy:3,blast:2,bounce:3},obstacles:[],shot:null,last:"你先手。先试试瞄准辅助，再拉弓。",hits:0,headshots:0,shots:0,score:0,assist:true};
  if(level%5!==0&&level>0)s.obstacles.push({x:290+(level%3)*14,y:238-level%4*12,width:22,height:105+level%4*12,kind:level%5===3?"wood":"stone",hp:2});
  if(level>=10&&level%3===0)s.obstacles.push({x:385,y:230,width:20,height:65,kind:"wood",hp:2});
  if(level>=20){s.obstacles.push({x:215,y:level%2?240:275,width:level%2?14:65,height:level%2?65:12,kind:level%2?"moving":"reflect",hp:2});}
  s.wind=level<5?0:Math.round((random(s)-.5)*(level<10?12:level<20?28:40));return s;
}
function makeShot(s:ArcheryState,side:ArcherSide,angle:number,power:number,kind:ArrowKind):ArcherShot {
  const p=archerPosition(s.level,side),sign=side==="player"?1:-1,rad=angle*Math.PI/180;
  const speed=power*(kind==="heavy"?.94:1);
  return {x:p.x+sign*17,y:p.y+16,vx:sign*Math.cos(rad)*speed,vy:-Math.sin(rad)*speed,owner:side,kind,bounces:0,age:0,trail:[]};
}
function hitArcher(shot:ArcherShot,p:{x:number;y:number}):"head"|"body"|null {
  if(Math.hypot(shot.x-p.x,shot.y-p.y)<=11)return "head";
  return Math.abs(shot.x-p.x)<11&&shot.y>=p.y+5&&shot.y<=p.y+46?"body":null;
}
function travel(s:ArcheryState,shot:ArcherShot,dt:number,apply:boolean):{kind:string;distance:number}|null {
  const steps=Math.max(1,Math.ceil(Math.hypot(shot.vx,shot.vy)*dt/3));
  for(let n=0;n<steps;n++){
    // 恒定风力和重力使用精确位移，避免预览与不同帧率下的飞行擦墙结果不一致。
    const d=dt/steps;shot.age+=d;shot.x+=shot.vx*d+s.wind*d*d/2;shot.y+=shot.vy*d+115*d*d;shot.vx+=s.wind*d;shot.vy+=230*d;
    const target=archerPosition(s.level,shot.owner==="player"?"enemy":"player");
    const hit=hitArcher(shot,target);
    if(hit)return {kind:hit,distance:0};
    for(const base of s.obstacles){if(base.hp<=0)continue;const o=obstacleAt(base,s.time+shot.age);
      if(shot.x>=o.x-2&&shot.x<=o.x+o.width+2&&shot.y>=o.y-2&&shot.y<=o.y+o.height+2){
        if(o.kind==="reflect"&&shot.kind==="bounce"&&shot.bounces===0){shot.vy=-Math.abs(shot.vy);shot.y=o.y-4;shot.bounces++;continue;}
        if(apply&&o.kind==="wood")base.hp-=shot.kind==="heavy"?2:1;
        return {kind:"wall",distance:Math.hypot(shot.x-target.x,shot.y-target.y-18)};
      }
    }
    if(shot.y>345||shot.x<0||shot.x>640||shot.age>6)return {kind:"miss",distance:Math.hypot(shot.x-target.x,shot.y-target.y-18)};
  }
  return null;
}
export function archeryTrajectory(s:ArcheryState,angle=s.angle,power=s.power,side:ArcherSide="player",kind:ArrowKind=s.kind){
  const shot=makeShot(s,side,angle,power,kind),points=[{x:shot.x,y:shot.y}];let result:null|{kind:string;distance:number}=null;
  for(let n=0;n<160&&!result;n++){result=travel(s,shot,.035,false);points.push({x:shot.x,y:shot.y});}
  return {points,result,shot};
}
/** 搜索同一套物理规则得到可行解；电脑再加入瞄准误差，不会穿墙命中。 */
export function suggestArcheryAim(s:ArcheryState,side:ArcherSide="player",kind:ArrowKind="normal") {
  let best={angle:48,power:350,error:1e9};
  const target=archerPosition(s.level,side==="player"?"enemy":"player");
  for(let angle=24;angle<=76;angle+=4)for(let power=250;power<=430;power+=12){
    const trace=archeryTrajectory(s,angle,power,side,kind);
    const error=trace.result?.distance??Math.hypot(trace.shot.x-target.x,trace.shot.y-target.y-22);
    const penalty=trace.result?.kind==="wall"?100:0;
    if(error+penalty<best.error)best={angle,power,error:error+penalty};
    if(error===0)return best;
  }return best;
}
export function shootArchery(s:ArcheryState,angle=s.angle,power=s.power,side:ArcherSide="player",kind:ArrowKind=s.kind){
  if((side==="player"&&s.phase!=="aim")||(side==="enemy"&&s.phase!=="ai")||!finite(angle,15,80)||!finite(power,180,450))return false;
  if(side==="player"&&kind!=="normal"&&s.ammo[kind]<=0)return false;
  if(side==="player"){if(kind!=="normal"){s.ammo[kind]--;if(s.ammo[kind]===0)s.kind="normal";}s.shots++;}
  s.shot=makeShot(s,side,angle,power,kind);s.phase="flight";s.last=side==="player"?"箭已离弦…":"电脑正在射击…";return true;
}
export function stepArchery(s:ArcheryState,dt:number){
  if(!finite(dt,.000001,.2)||s.phase==="won"||s.phase==="lost")return;
  if(s.phase==="ai"){
    s.aiTime+=dt;if(s.aiTime<.85)return;s.aiTime=0;
    const aim=suggestArcheryAim(s,"enemy");
    const uncertainty=s.level<10?6:s.level<20?4.5:3.2;
    shootArchery(s,clamp(aim.angle+(random(s)-.5)*uncertainty*2,15,80),clamp(aim.power+(random(s)-.5)*35,180,450),"enemy","normal");return;
  }
  if(s.phase!=="flight"||!s.shot)return;
  const shot=s.shot;const result=travel(s,shot,dt,true);shot.trail.push({x:shot.x,y:shot.y});shot.trail=shot.trail.slice(-50);
  if(!result)return;
  let damage=0;
  if(result.kind==="head"||result.kind==="body"){
    const head=result.kind==="head";damage=shot.owner==="player"?(head?64:36)+(shot.kind==="heavy"?10:0):(head?31:22);
    if(shot.owner==="player"){s.hits++;if(head)s.headshots++;}s.last=head?"漂亮，命中头部！":"命中！";
  }else s.last=result.kind==="wall"?"箭被障碍挡住，试试更高的弧线。":"差一点，再修正角度试试。";
  if(shot.kind==="blast"){
    for(const o of s.obstacles)if(o.kind==="wood"&&Math.hypot(o.x+o.width/2-shot.x,o.y+o.height/2-shot.y)<90)o.hp=0;
    const target=archerPosition(s.level,shot.owner==="player"?"enemy":"player");
    if(Math.hypot(shot.x-target.x,shot.y-target.y-20)<75)damage=Math.max(damage,42);
  }
  if(shot.owner==="player")s.enemyHP=Math.max(0,s.enemyHP-damage);else s.playerHP=Math.max(0,s.playerHP-damage);
  if(s.enemyHP===0){s.phase="won";s.score=200+s.level*30+s.playerHP*2+s.headshots*50;}
  else if(s.playerHP===0)s.phase="lost";
  else{s.phase=shot.owner==="player"?"ai":"aim";s.turn++;s.time+=shot.age;s.wind=s.level<5?0:Math.round((random(s)-.5)*(s.level<10?12:s.level<20?28:40));}
  s.shot=null;
}
export function validArchery(v:unknown):v is ArcheryState {
  if(!object(v)||v.version!==1||!["campaign","endless"].includes(v.mode as string)||!integer(v.level,0,29)||!integer(v.seed,0,4294967295)||!["aim","flight","ai","won","lost"].includes(v.phase as string))return false;
  if(!["normal","heavy","blast","bounce"].includes(v.kind as string)||!object(v.ammo)||!ARROWS.every(a=>integer((v.ammo as Record<string,unknown>)[a.id],0,999)))return false;
  if(![v.playerHP,v.enemyHP,v.enemyMax,v.time,v.aiTime,v.turn,v.hits,v.headshots,v.shots,v.score].every(n=>finite(n,0,1e8))||!finite(v.wind,-25,25)||!finite(v.angle,15,80)||!finite(v.power,180,450)||typeof v.assist!=="boolean"||typeof v.last!=="string")return false;
  if(!Array.isArray(v.obstacles)||v.obstacles.length>5||!v.obstacles.every(o=>object(o)&&[o.x,o.y,o.width,o.height,o.hp].every(n=>finite(n,-5,640))&&["wood","stone","moving","reflect"].includes(o.kind as string)))return false;
  if(v.shot!==null){const q=v.shot;if(!object(q)||![q.x,q.y,q.vx,q.vy].every(n=>finite(n,-3000,3000))||!finite(q.age,0,7)||!integer(q.bounces,0,1)||!["player","enemy"].includes(q.owner as string)||!ARROWS.some(a=>a.id===q.kind)||!Array.isArray(q.trail)||q.trail.length>50||!q.trail.every(p=>object(p)&&finite(p.x,-3000,3000)&&finite(p.y,-3000,3000)))return false;}
  return (v.phase==="flight")===(v.shot!==null);
}
