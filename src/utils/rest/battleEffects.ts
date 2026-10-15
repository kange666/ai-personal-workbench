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
}

// 连射、爆破和群怪时仍限制节点数，避免特效无限积累。
export const BATTLE_EFFECT_LIMIT = 24;
export function addBattleEffect(effects: BattleEffect[], effect: BattleEffect) {
  effects.push(effect);
  if (effects.length > BATTLE_EFFECT_LIMIT) effects.splice(0, effects.length - BATTLE_EFFECT_LIMIT);
}
export function stepBattleEffects(effects: BattleEffect[], seconds: number) {
  for (const effect of effects) effect.life = Math.max(0, effect.life - seconds);
  return effects.filter(effect => effect.life > 0);
}
export function effectProgress(effect: BattleEffect) {
  return 1 - effect.life / effect.duration;
}
