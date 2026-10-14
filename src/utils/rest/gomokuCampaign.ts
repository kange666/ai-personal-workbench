import { createGomoku, isGomokuState, placeGomoku, winningLine, type GomokuDifficulty, type GomokuState } from "./gomoku";
import { createProgress, object, validProgress, type GameProgress } from "./progression";
export const GOMOKU_PUZZLES = Array.from({length:30},(_,i)=>({name:["一步连五","封住唯一缺口","制造双四"][Math.floor(i/10)]!+` ${i%10+1}`,chapter:["进攻","防守","双重威胁"][Math.floor(i/10)]!}));
export function createGomokuPuzzle(index: number) {
  const i=index%10, kind=Math.floor(index/10), x=3+i%4, y=3+Math.floor(i/4)*3;
  let black:number[],white:number[],answer:number;
  if(kind<2) {
    const line=Array.from({length:4},(_,n)=>y*15+x+n);
    answer=y*15+x+4;
    if(kind===0) {black=line;white=[180,182,184,186];}
    else {white=line;black=[y*15+x-1,0,2,4];}
  } else {
    const cx=6+i%3,cy=6+Math.floor(i/3); answer=cy*15+cx;
    black=[1,2,3].flatMap(n=>[cy*15+cx-n,(cy-n)*15+cx]);white=[195,197,199,201,203,205];
  }
  const run=createGomoku();
  black.forEach((cell,i)=>{placeGomoku(run,cell,1);placeGomoku(run,white[i]!,2);});
  return {run,answer};
}
export function correctGomokuPuzzle(index:number, before:GomokuState, move:number) {
  const {answer}=createGomokuPuzzle(index);
  if(index>=10) return move===answer;
  const board=[...before.board];board[move]=1;return winningLine(board,move).length>=5;
}
export interface GomokuSession {version:2;run:GomokuState;difficulty:GomokuDifficulty;mode:"endless"|"puzzle";level:number;done:boolean;hintUsed?:boolean;progress:GameProgress}
export function createGomokuSession():GomokuSession{return {version:2,run:createGomoku(),difficulty:"relaxed",mode:"endless",level:0,done:false,hintUsed:false,progress:createProgress(30)};}
export function validGomokuSession(v:unknown):v is GomokuSession {
  return object(v)&&v.version===2&&isGomokuState(v.run)&&["relaxed","normal","hard","expert"].includes(v.difficulty as string)&&["endless","puzzle"].includes(v.mode as string)&&Number.isInteger(v.level)&&Number(v.level)>=0&&Number(v.level)<30&&typeof v.done==="boolean"&&(v.hintUsed===undefined||typeof v.hintUsed==="boolean")&&validProgress(v.progress,30);
}
export function migrateGomoku(v:unknown):GomokuSession|null{return isGomokuState(v)?{...createGomokuSession(),run:v}:null;}
