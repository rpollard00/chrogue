// A square is an index from 0 (a1) to 63 (h8). White moves toward higher ranks.
export type Square = number;
export type Color = 'w' | 'b';
export type PieceType = 'p' | 'n' | 'b' | 'r' | 'q' | 'k';
export type PromotionType = 'q' | 'n' | 'r' | 'b';
export type PieceId = number | string;

export interface Piece {
  id: PieceId;
  type: PieceType;
  color: Color;
  moved: boolean;
}

export interface PieceSetup {
  id: PieceId;
  type: PieceType;
  color: Color;
  square: Square;
  moved?: boolean;
}

// The movement rules that a side can add to the usual chess rules.
export interface MoveRules {
  /** Pawns can always move two squares forward. */
  forcedMarch: boolean;
  /** Pawns can move one square backward to an empty square. */
  backpedal: boolean;
  /** Pawns promote one rank earlier. */
  earlyPromo: boolean;
  /** The king can also move as a knight. */
  kingKnight: boolean;
  /** Knights can also jump three squares in one direction and one square to the side. */
  longLeap: boolean;
  /** Bishops can move one square orthogonally to an empty square. */
  sidestep: boolean;
}
export type RuleSet = Partial<MoveRules>;

export interface Move {
  from: Square;
  to: Square;
  promo?: PromotionType;
  /** The en passant target square that this move makes. */
  ep?: Square;
  epCapture?: true;
  /** The rook move of a castle: [from, to]. */
  castle?: [Square, Square];
}

export interface State {
  board: (Piece | null)[];
  turn: Color;
  ep: Square;
  /** The number of half moves with no capture. */
  clock: number;
  rules: Record<Color, RuleSet>;
}

export interface Undo {
  captured: Piece | null;
  capSq: Square;
  ep: Square;
  clock: number;
  moved: boolean;
  type: PieceType;
}

export type Outcome =
  | { winner: Color; reason: 'checkmate' | 'stalemate' | 'rout' }
  | { winner: null; reason: 'bare' | 'clock' };

export const VALUE: Record<PieceType, number> = { p: 1, n: 3, b: 3, r: 5, q: 9, k: 0 };
export const other = (color: Color): Color => (color === 'w' ? 'b' : 'w');
