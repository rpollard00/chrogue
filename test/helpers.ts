import { createState } from '../src/engine';
import type { Color, PieceSetup, PieceType, RuleSet, Square, State } from '../src/engine';

export const sq = (name: string): Square => 'abcdefgh'.indexOf(name[0]) + 8 * (Number(name[1]) - 1);

// Reads the piece field of a FEN string. A pawn off its start rank counts as moved.
export function fromFen(fen: string, turn: Color = 'w', rules?: Partial<Record<Color, RuleSet>>): State {
  const pieces: PieceSetup[] = [];
  fen.split('/').forEach((row, i) => {
    let f = 0;
    for (const ch of row) {
      if (ch >= '1' && ch <= '8') {
        f += Number(ch);
        continue;
      }
      const color: Color = ch === ch.toUpperCase() ? 'w' : 'b', type = ch.toLowerCase() as PieceType, r = 7 - i;
      const moved = type === 'p' && r !== (color === 'w' ? 1 : 6);
      pieces.push({ id: pieces.length, type, color, square: r * 8 + f++, moved });
    }
  });
  const state = createState(pieces, rules);
  state.turn = turn;
  return state;
}
