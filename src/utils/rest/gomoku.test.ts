import { describe, expect, it } from "vitest";
import { chooseGomokuMove, createGomoku, isGomokuState, placeGomoku, undoGomoku, winningLine, type Stone } from "./gomoku";

describe("五子棋规则与存档", () => {
  it.each([
    [0, 1, 2, 3, 4],
    [0, 15, 30, 45, 60],
    [0, 16, 32, 48, 64],
    [14, 28, 42, 56, 70],
  ])("四个方向在棋盘边缘也能正确识别连五 %j", (...indices: number[]) => {
    const state = createGomoku();
    indices.forEach((index, turn) => {
      expect(placeGomoku(state, index, 1)).toBe(true);
      if (turn < 4) expect(placeGomoku(state, 200 + turn * 2, 2)).toBe(true);
    });
    expect(state.status).toBe("won");
    expect(state.score).toBe(450);
    expect(winningLine(state.board, indices[4]!)).toHaveLength(5);
    expect(isGomokuState(JSON.parse(JSON.stringify(state)))).toBe(true);
    expect(placeGomoku(state, 100, 2)).toBe(false);
    expect(undoGomoku(state)).toBe(false);
  });

  it("不把跨行排列误认为横向五子", () => {
    const board: Stone[] = Array<Stone>(225).fill(0);
    for (const index of [13, 14, 15, 16, 17]) board[index] = 1;
    expect(winningLine(board, 15)).toEqual([]);
  });

  it("只允许按顺序下在空位，悔棋退回到玩家回合", () => {
    const state = createGomoku();
    expect(placeGomoku(state, 112, 2)).toBe(false);
    expect(placeGomoku(state, -1, 1)).toBe(false);
    expect(placeGomoku(state, 225, 1)).toBe(false);
    expect(placeGomoku(state, 1.2, 1)).toBe(false);
    placeGomoku(state, 112, 1);
    expect(placeGomoku(state, 112, 2)).toBe(false);
    expect(undoGomoku(state)).toBe(true);
    expect(state.moves).toHaveLength(0);
    placeGomoku(state, 112, 1);
    placeGomoku(state, 113, 2);
    expect(undoGomoku(state)).toBe(true);
    expect(state.board.every(value => value === 0)).toBe(true);
    expect(undoGomoku(state)).toBe(false);
  });

  it("电脑回合的存档保持完整，继续落子后不会再次成为电脑回合", () => {
    const state = createGomoku();
    placeGomoku(state, 112, 1);
    const restored = JSON.parse(JSON.stringify(state));
    expect(isGomokuState(restored)).toBe(true);
    expect(restored.moves.length % 2).toBe(1);
    const next = chooseGomokuMove(restored.board);
    expect(next).not.toBeNull();
    expect(placeGomoku(restored, next!, 2)).toBe(true);
    expect(placeGomoku(restored, 114, 2)).toBe(false);
    expect(isGomokuState(restored)).toBe(true);
  });

  it("棋谱与棋盘、胜负和得分必须一致，拒绝篡改和重复落子", () => {
    const state = createGomoku();
    placeGomoku(state, 112, 1);
    expect(isGomokuState(state)).toBe(true);
    for (const invalid of [null, {}, { ...state, board: [] }, { ...state, score: 100 }, { ...state, status: "won" }, { ...state, moves: [{ index: 112, player: 2 }] }, { ...state, moves: [...state.moves, { index: 112, player: 2 }] }, { ...state, board: state.board.map(() => 0) }]) expect(isGomokuState(invalid)).toBe(false);
  });

  it("棋盘填满但没有连五时正确判为和局", () => {
    const state = createGomoku();
    // 两行一换的双色斜纹不会在横、竖或斜线上出现连续五子。
    const board: Stone[] = Array.from({ length: 225 }, (_, index) => ((index % 15 + Math.floor(Math.floor(index / 15) / 2)) % 2 === 0 ? 1 : 2));
    const black = board.flatMap((stone, index) => stone === 1 ? [index] : []);
    const white = board.flatMap((stone, index) => stone === 2 ? [index] : []);
    for (let turn = 0; turn < 225; turn += 1) expect(placeGomoku(state, (turn % 2 === 0 ? black : white)[Math.floor(turn / 2)]!, turn % 2 === 0 ? 1 : 2)).toBe(true);
    expect(state.status).toBe("draw");
    expect(state.score).toBe(50);
    expect(isGomokuState(state)).toBe(true);
    expect(chooseGomokuMove(state.board)).toBeNull();
  });
});

describe("本地电脑落子", () => {
  it("优先自己取胜，然后封堵黑棋四连", () => {
    const board: Stone[] = Array<Stone>(225).fill(0);
    for (const index of [105, 106, 107, 108]) board[index] = 1;
    expect(chooseGomokuMove(board)).toBe(109);
    for (const index of [30, 31, 32, 33]) board[index] = 2;
    expect(chooseGomokuMove(board)).toBe(34);
  });

  it("封堵中间缺口，评估不会修改输入棋盘", () => {
    const board: Stone[] = Array<Stone>(225).fill(0);
    for (const index of [105, 106, 108, 109]) board[index] = 1;
    const before = [...board];
    expect(chooseGomokuMove(board)).toBe(107);
    expect(board).toEqual(before);
  });

  it("棋盘为空选择中心；普通回合只选择合法空位", () => {
    const state = createGomoku();
    expect(chooseGomokuMove(state.board)).toBe(112);
    placeGomoku(state, 112, 1);
    const chosen = chooseGomokuMove(state.board)!;
    expect(state.board[chosen]).toBe(0);
    expect(placeGomoku(state, chosen, 2)).toBe(true);
  });

  it("多局完整对弈都能合法结束，任意回合存档可恢复", () => {
    for (let game = 0; game < 12; game += 1) {
      const state = createGomoku();
      let random = game + 17;
      while (state.status === "playing") {
        random = (random * 16807) % 2147483647;
        const empty = state.board.flatMap((stone, index) => stone === 0 ? [index] : []);
        const player = state.moves.length % 2 === 0 ? 1 : 2;
        const index = player === 1 ? empty[random % empty.length]! : chooseGomokuMove(state.board)!;
        expect(placeGomoku(state, index, player)).toBe(true);
        expect(isGomokuState(state)).toBe(true);
      }
      expect(["won", "lost", "draw"]).toContain(state.status);
    }
  });
});
