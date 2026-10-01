// Saved data in localStorage. This module checks the data that it reads.
import { isRelicId } from './relics';
import { FLOORS } from './floors';
import type { Enemy, Meta, Offer, Run, Unit } from './types';
import { isUpgradeId } from './upgrades';

const META_KEY = 'chrogue.meta';
const RUN_KEY = 'chrogue.run';
const PIECE_TYPES = 'pnbrqk';

type Json = Record<string, unknown>;
const isObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value);
const isCount = (value: unknown): value is number => Number.isInteger(value) && (value as number) >= 0;
const isPieceType = (value: unknown): value is Unit['type'] =>
  typeof value === 'string' && value.length === 1 && PIECE_TYPES.includes(value);
const isSquare = (value: unknown, max: number): value is number => isCount(value) && value <= max;
const list = (value: unknown): unknown[] => (Array.isArray(value) ? value : []);

function read(key: string): unknown {
  try {
    return JSON.parse(localStorage.getItem(key) ?? 'null');
  } catch {
    return null;
  }
}

export function parseMeta(raw: unknown): Meta {
  const data = isObject(raw) ? raw : {};
  const meta: Meta = { crowns: 0, best: 0, runs: 0, upgrades: {} };
  if (isCount(data.crowns)) meta.crowns = data.crowns;
  if (isCount(data.best)) meta.best = data.best;
  if (isCount(data.runs)) meta.runs = data.runs;
  for (const [id, level] of Object.entries(isObject(data.upgrades) ? data.upgrades : {})) {
    if (isUpgradeId(id) && isCount(level)) meta.upgrades[id] = level;
  }
  return meta;
}

function parseOffer(raw: unknown): Offer | null {
  if (!isObject(raw)) return null;
  if (raw.kind === 'piece' && isPieceType(raw.type) && raw.type !== 'k') return { kind: 'piece', type: raw.type };
  if (raw.kind === 'relic' && isRelicId(raw.id)) return { kind: 'relic', id: raw.id };
  if (raw.kind === 'gold' && isCount(raw.amount)) return { kind: 'gold', amount: raw.amount };
  return null;
}

const parseOffers = (raw: unknown): Offer[] => list(raw).map(parseOffer).filter((offer) => offer !== null);

// Returns null if the data is not a run that the game can continue.
export function parseRun(raw: unknown): Run | null {
  if (!isObject(raw) || !isObject(raw.enemy)) return null;
  const { floor, gold, nextId, enemy } = raw;
  if (!isCount(floor) || floor < 1 || floor > FLOORS.length || !isCount(gold) || !isCount(nextId)) return null;

  const army: Unit[] = [];
  for (const unit of list(raw.army)) {
    if (!isObject(unit) || !isCount(unit.id) || !isPieceType(unit.type) || !isSquare(unit.home, 15)) return null;
    army.push({ id: unit.id, type: unit.type, home: unit.home });
  }
  const pieces: Enemy['pieces'] = [];
  for (const piece of list(enemy.pieces)) {
    if (!isObject(piece) || !isPieceType(piece.type) || !isSquare(piece.square, 63)) return null;
    pieces.push({ type: piece.type, square: piece.square });
  }
  const hasKing = (types: { type: string }[]) => types.filter((p) => p.type === 'k').length === 1;
  if (!hasKing(army) || !hasKing(pieces) || new Set(army.map((u) => u.home)).size !== army.length) return null;

  return {
    floor, gold, nextId, army,
    relics: list(raw.relics).filter(isRelicId),
    enemy: { pieces, traits: list(enemy.traits).filter(isRelicId) },
    phase: raw.phase === 'camp' ? 'camp' : 'battle',
    draft: raw.draft == null ? null : parseOffers(raw.draft),
    shop: parseOffers(raw.shop),
  };
}

export const loadMeta = (): Meta => parseMeta(read(META_KEY));
export const loadRun = (): Run | null => parseRun(read(RUN_KEY));
export const saveMeta = (meta: Meta): void => localStorage.setItem(META_KEY, JSON.stringify(meta));

export function saveRun(run: Run | null): void {
  if (run) localStorage.setItem(RUN_KEY, JSON.stringify(run));
  else localStorage.removeItem(RUN_KEY);
}
