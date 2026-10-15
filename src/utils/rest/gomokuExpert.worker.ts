/// <reference lib="webworker" />
import { searchGomokuDeepSeek, searchGomokuExpert } from './gomokuExpert';
import type { Stone } from './gomoku';

self.onmessage = (event: MessageEvent<{ board: Stone[]; side: 1 | 2; strongest?: boolean; preferred?:number }>) => {
  // 单独计算线程保持棋盘、暂停与关闭按钮响应；页面结束思考时直接终止本线程。
  self.postMessage(event.data.strongest ? searchGomokuDeepSeek(event.data.board,event.data.side,8000,event.data.preferred) : searchGomokuExpert(event.data.board, event.data.side));
};
