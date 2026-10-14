import { integer, object, seeded } from "./progression";
export const WIRING_LEVELS=Array.from({length:60},(_,i)=>({name:["线路起步","转角连接","分支汇流","环路检查","全部点亮"][i%5]!+` ${Math.floor(i/5)+1}`,chapter:["基础电路","固定接点","双色供电","交叉桥接"][Math.floor(i/15)]!}));
const dirs=[{dx:0,dy:-1,bit:1,back:4},{dx:1,dy:0,bit:2,back:8},{dx:0,dy:1,bit:4,back:1},{dx:-1,dy:0,bit:8,back:2}];
export const rotateCircuit=(mask:number)=>((mask<<1)&15)|(mask>>3);
export interface Circuit {mode:'campaign'|'endless';level:number;seed:number;size:number;tiles:number[];solution:number[];locked:boolean[];bridges:boolean[];sources:{index:number;color:number}[];moves:number;hints:number;history:number[][];won:boolean}
export function createCircuit(level:number,seed=level+17001):Circuit {
  const random=seeded(seed),chapter=Math.floor(level/15),size=chapter===0?(level<5?4:5):chapter===1?5:chapter===2?6:7;
  const solution=Array<number>(size*size).fill(0),bridges=Array<boolean>(size*size).fill(false);
  const connect=(a:number,b:number)=>{const dx=b%size-a%size,dy=Math.floor(b/size)-Math.floor(a/size),dir=dirs.find(d=>d.dx===dx&&d.dy===dy)!;solution[a]!|=dir.bit;solution[b]!|=dir.back;};
  function tree(cells:number[]){const allowed=new Set(cells),seen=new Set<number>(),stack=[cells[0]!];seen.add(cells[0]!);while(stack.length){const current=stack.at(-1)!,x=current%size,y=Math.floor(current/size);const choices=dirs.map(d=>({x:x+d.dx,y:y+d.dy})).filter(p=>p.x>=0&&p.x<size&&p.y>=0&&p.y<size).map(p=>p.y*size+p.x).filter(n=>allowed.has(n)&&!seen.has(n));if(!choices.length){stack.pop();continue;}const next=choices[Math.floor(random()*choices.length)]!;connect(current,next);seen.add(next);stack.push(next);}}
  let sources:{index:number;color:number}[];
  if(chapter<2){tree(solution.map((_,i)=>i));sources=[{index:Math.floor(size*size/2),color:1}];}
  else if(chapter===2){tree(solution.flatMap((_,i)=>i%size<size/2?[i]:[]));tree(solution.flatMap((_,i)=>i%size>=size/2?[i]:[]));sources=[{index:0,color:1},{index:size-1,color:2}];}
  else {
    const mid=Math.floor(size/2);for(let i=0;i<size-1;i++){connect(i*size+mid,(i+1)*size+mid);connect(mid*size+i,mid*size+i+1);}
    for(const sx of [-1,1])for(const sy of [-1,1]){const cells=solution.flatMap((_,i)=>((i%size-mid)*sx>0&&(Math.floor(i/size)-mid)*sy>0)?[i]:[]);tree(cells);const corner=(mid+sy)*size+mid+sx;connect(corner,sx===sy?(mid+sy)*size+mid:mid*size+mid+sx);}
    bridges[mid*size+mid]=true;sources=[{index:mid,color:1},{index:mid*size,color:2}];
  }
  const locked=solution.map((_,i)=>bridges[i]||(chapter>0&&random()<.12));
  const tiles=solution.map((mask,i)=>{if(locked[i])return mask;let value=mask;for(let n=Math.floor(random()*4);n>0;n--)value=rotateCircuit(value);return value;});
  const circuit:Circuit={mode:"campaign",level,seed,size,tiles,solution,locked,bridges,sources,moves:0,hints:0,history:[],won:false};
  if(circuitStatus(circuit).won){const i=tiles.findIndex((m,i)=>!locked[i]&&rotateCircuit(m)!==m);if(i>=0)tiles[i]=rotateCircuit(tiles[i]!);}
  return circuit;
}
export function circuitStatus(s:Circuit){
  const colors=Array<number>(s.size*s.size*2).fill(0),queue:number[]=[];let leaks=0,mixed=false;
  const groups=(i:number)=>s.bridges[i]?[5,10]:[s.tiles[i]!];
  for(const source of s.sources){colors[source.index*2]=source.color;queue.push(source.index*2);}
  const neighbor=(i:number,d:typeof dirs[number])=>{const x=i%s.size+d.dx,y=Math.floor(i/s.size)+d.dy;return x>=0&&x<s.size&&y>=0&&y<s.size?y*s.size+x:-1;};
  for(let i=0;i<s.tiles.length;i++)for(const d of dirs)if(s.tiles[i]!&d.bit){const next=neighbor(i,d);if(next<0||!(s.tiles[next]!&d.back))leaks++;}
  for(let cursor=0;cursor<queue.length;cursor++){
    const node=queue[cursor]!,index=Math.floor(node/2),group=groups(index)[node%2]||0;
    for(const d of dirs)if(group&d.bit){const next=neighbor(index,d);if(next<0||!(s.tiles[next]!&d.back))continue;const target=next*2+(s.bridges[next]&&[2,8].includes(d.back)?1:0);
      if(colors[target]&&colors[target]!==colors[node])mixed=true;
      if(!colors[target]){colors[target]=colors[node]!;queue.push(target);}
    }
  }
  const powered=s.tiles.filter((_,i)=>groups(i).every((_,n)=>colors[i*2+n])).length;
  return{colors,leaks,mixed,powered,won:powered===s.tiles.length&&leaks===0&&!mixed};
}
export function turnCircuit(s:Circuit,index:number,hint=false){if(s.won||!integer(index,0,s.tiles.length-1)||s.locked[index])return;s.history.push([...s.tiles]);s.history=s.history.slice(-100);s.tiles[index]=rotateCircuit(s.tiles[index]!);s.moves++;if(hint)s.hints++;s.won=circuitStatus(s).won;}
export function hintCircuit(s:Circuit){const index=s.tiles.findIndex((mask,i)=>!s.locked[i]&&mask!==s.solution[i]);if(index>=0)turnCircuit(s,index,true);return index;}
export function undoCircuit(s:Circuit){const previous=s.history.pop();if(previous){s.tiles=previous;s.moves=Math.max(0,s.moves-1);s.won=false;}}
export function circuitStars(s:Circuit){return s.hints===0&&s.moves<=s.size*s.size*2?3:s.hints<=3?2:1;}
export function validCircuit(v:unknown):v is Circuit {
  if(!object(v)||!["campaign","endless"].includes(v.mode as string)||!integer(v.level,0,59)||!integer(v.seed,0,4294967295)||!integer(v.moves)||!integer(v.hints)||typeof v.won!=="boolean")return false;
  const reference=createCircuit(v.level,v.seed);
  if(v.size!==reference.size||!Array.isArray(v.tiles)||v.tiles.length!==reference.tiles.length||!v.tiles.every((n,i)=>integer(n,1,15)&&[reference.solution[i],rotateCircuit(reference.solution[i]!),rotateCircuit(rotateCircuit(reference.solution[i]!)),rotateCircuit(rotateCircuit(rotateCircuit(reference.solution[i]!)))].includes(n)&&(!reference.locked[i]||n===reference.solution[i])))return false;
  if(JSON.stringify(v.solution)!==JSON.stringify(reference.solution)||JSON.stringify(v.sources)!==JSON.stringify(reference.sources)||JSON.stringify(v.locked)!==JSON.stringify(reference.locked)||JSON.stringify(v.bridges)!==JSON.stringify(reference.bridges))return false;
  return Array.isArray(v.history)&&v.history.length<=100&&v.history.every(row=>Array.isArray(row)&&row.length===reference.tiles.length&&row.every((n,i)=>integer(n,1,15)&&[0,1,2,3].some(r=>{let mask=reference.solution[i]!;for(let k=0;k<r;k++)mask=rotateCircuit(mask);return n===mask;})&&(!reference.locked[i]||n===reference.solution[i])))&&v.won===circuitStatus(v as unknown as Circuit).won;
}
