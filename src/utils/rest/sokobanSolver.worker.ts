import { solveSokoban, type SokobanBoard, type SokobanSnapshot } from './sokoban';
self.onmessage = (event: MessageEvent<{ board: SokobanBoard; state: SokobanSnapshot }>) => {
  self.postMessage(solveSokoban(event.data.board, event.data.state));
};
