// Rewards and shop items. To add a kind of offer:
// 1. Add its data to the Offer type in types.ts.
// 2. Add one entry to OFFER_KINDS. The compiler reports a missing entry.
// 3. Add it to draftPool or shopStock so that the game can offer it.
import { PIECE_NAME, addUnit, canAddUnit } from './army';
import { pickWeighted, shuffle } from './random';
import type { Weighted } from './random';
import { RELICS, RELIC_IDS } from './relics';
import type { Offer, RecruitType, Run } from './types';

export type OfferIcon = RecruitType | 'relic' | 'gold';

export interface OfferInfo {
  icon: OfferIcon;
  name: string;
  text: string;
}

interface OfferKind<O extends Offer> {
  describe(offer: O): OfferInfo;
  /** Returns the reason that the run cannot take the offer, or null. */
  blocked(offer: O, run: Run): string | null;
  take(offer: O, run: Run): void;
  /** The shop price in gold before upgrades. */
  price(offer: O): number;
}

const PIECE_PRICE: Record<RecruitType, number> = { p: 5, n: 13, b: 13, r: 20, q: 34 };
const RELIC_PRICE = 16;

const OFFER_KINDS: { [K in Offer['kind']]: OfferKind<Extract<Offer, { kind: K }>> } = {
  piece: {
    describe: ({ type }) => ({
      icon: type,
      name: PIECE_NAME[type],
      text: `Add this ${PIECE_NAME[type].toLowerCase()} to your army.`,
    }),
    blocked: (_, run) => (canAddUnit(run) ? null : 'Your army is full.'),
    take: ({ type }, run) => void addUnit(run, type),
    price: ({ type }) => PIECE_PRICE[type],
  },
  relic: {
    describe: ({ id }) => ({ icon: 'relic', name: RELICS[id].name, text: RELICS[id].text }),
    blocked: ({ id }, run) => (run.relics.includes(id) ? 'You have this relic.' : null),
    take: ({ id }, run) => void run.relics.push(id),
    price: () => RELIC_PRICE,
  },
  gold: {
    describe: ({ amount }) => ({ icon: 'gold', name: `${amount} gold`, text: `Get ${amount} gold.` }),
    blocked: () => null,
    take: ({ amount }, run) => void (run.gold += amount),
    price: () => 0,
  },
};

// The compiler cannot relate offer.kind to the entry type, thus this function has the one cast.
const kindOf = <O extends Offer>(offer: O) => OFFER_KINDS[offer.kind] as unknown as OfferKind<O>;

export const describeOffer = (offer: Offer): OfferInfo => kindOf(offer).describe(offer);
export const blockedReason = (offer: Offer, run: Run): string | null => kindOf(offer).blocked(offer, run);
export const basePrice = (offer: Offer): number => kindOf(offer).price(offer);

// Returns false if the run cannot take the offer.
export function takeOffer(offer: Offer, run: Run): boolean {
  if (blockedReason(offer, run)) return false;
  kindOf(offer).take(offer, run);
  return true;
}

const RECRUITS: { type: RecruitType; weight: number; minFloor: number }[] = [
  { type: 'p', weight: 3, minFloor: 1 },
  { type: 'n', weight: 3, minFloor: 1 },
  { type: 'b', weight: 3, minFloor: 1 },
  { type: 'r', weight: 1.5, minFloor: 1 },
  { type: 'q', weight: 0.5, minFloor: 3 },
];

const piecePool = (floor: number): Weighted<Offer>[] =>
  RECRUITS.filter((r) => floor >= r.minFloor).map((r) => ({ item: { kind: 'piece', type: r.type }, weight: r.weight }));

const newRelics = (run: Run, n: number): Offer[] =>
  shuffle(RELIC_IDS.filter((id) => !run.relics.includes(id))).slice(0, n).map((id) => ({ kind: 'relic', id }));

// The three free rewards after a win. run.floor is the floor that comes next.
export function rollDraft(run: Run): Offer[] {
  return pickWeighted<Offer>([
    ...piecePool(run.floor),
    ...newRelics(run, 2).map((item) => ({ item, weight: 2 })),
    { item: { kind: 'gold', amount: 10 + 2 * run.floor }, weight: 2 },
  ], 3);
}

export function rollShop(run: Run): Offer[] {
  return [...pickWeighted(piecePool(run.floor), 2), ...newRelics(run, 2)];
}
