/** 仅供画面使用的短动画，不进入存档，也不参与命中或伤害计算。 */
export interface BattleEffect {
  id: number;
  kind: 'hit' | 'coin' | 'upgrade' | 'shot' | 'chain' | 'burst';
  x: number;
  y: number;
  life: number;
  duration: number;
  color: string;
  text?: string;
  target?: number;
  fromX?: number;
  fromY?: number;
  /** 仅影响绘制的材质和覆盖圈，实际次数与半径由战斗规则计算。 */
  variant?: 'kinetic' | 'shell' | 'fire' | 'ice' | 'plasma' | 'shield' | 'nova' | 'volley' | 'pulse' | 'beam';
  radius?: number;
  amount?: number;
  /** 延后命中反馈以衔接弹道；不延迟规则层伤害或存档。 */
  delay?: number;
}

// 连射、爆破和群怪时仍限制节点数，避免特效无限积累。
export const BATTLE_EFFECT_LIMIT = 24;
export function addBattleEffect(effects: BattleEffect[], effect: BattleEffect) {
  effects.push(effect);
  if (effects.length > BATTLE_EFFECT_LIMIT) effects.splice(0, effects.length - BATTLE_EFFECT_LIMIT);
}
export function stepBattleEffects(effects: BattleEffect[], seconds: number) {
  for (const effect of effects) {
    const delay = effect.delay || 0;
    if (delay) effect.delay = Math.max(0, delay - seconds);
    effect.life = Math.max(0, effect.life - Math.max(0, seconds - delay));
  }
  return effects.filter(effect => effect.life > 0);
}
export function effectProgress(effect: BattleEffect) {
  return 1 - effect.life / effect.duration;
}
