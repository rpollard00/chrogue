// Game data: relics, floors, upgrades, prices, and the random generators.

export const PIECE_NAME = { k: 'King', q: 'Queen', r: 'Rook', b: 'Bishop', n: 'Knight', p: 'Pawn' };
export const ARMY_MAX = 16;
export const REROLL_COST = 3;

const PIECE_VALUE = { p: 1, n: 3, b: 3, r: 5, q: 9 };
const PIECE_PRICE = { p: 5, n: 13, b: 13, r: 20, q: 34 };
const RELIC_PRICE = 16;

// A relic with a foe text is also an engine flag, and a boss can have it as a trait.
export const RELICS = [
  {
    id: 'forcedMarch', name: 'Forced March',
    text: 'Your pawns can always move two squares forward.',
    foe: 'Enemy pawns can always move two squares forward.',
  },
  {
    id: 'backpedal', name: 'Tactical Retreat',
    text: 'Your pawns can move one square backward to an empty square.',
    foe: 'Enemy pawns can move one square backward to an empty square.',
  },
  {
    id: 'earlyPromo', name: 'Field Promotion',
    text: 'Your pawns promote one rank earlier.',
    foe: 'Enemy pawns promote one rank earlier.',
  },
  {
    id: 'kingKnight', name: 'Royal Steed',
    text: 'Your king can also move as a knight.',
    foe: 'The enemy king can also move as a knight.',
  },
  {
    id: 'longLeap', name: 'Long Leap',
    text: 'Your knights can also jump three squares in one direction and one square to the side.',
    foe: 'Enemy knights can also jump three squares in one direction and one square to the side.',
  },
  {
    id: 'sidestep', name: 'Sidestep',
    text: 'Your bishops can move one square up, down, left, or right to an empty square.',
    foe: 'Enemy bishops can move one square up, down, left, or right to an empty square.',
  },
  { id: 'bounty', name: 'Bounty', text: 'You get 50% more gold for each capture.' },
  {
    id: 'secondWind', name: 'Second Wind',
    text: 'The first piece that you lose in each battle returns after the battle.',
  },
  {
    id: 'conscription', name: 'Conscription',
    text: 'You start each battle with one more pawn. The pawn leaves after the battle.',
  },
  {
    id: 'interest', name: 'Interest',
    text: 'After each battle that you win, you get 1 gold for each 5 gold that you have. The maximum is 6 gold.',
  },
];
export const RELIC_BY_ID = Object.fromEntries(RELICS.map((r) => [r.id, r]));

// budget: total piece value of the enemy army. depth and noise set the AI strength.
export const FLOORS = [
  { name: 'Border Patrol', budget: 5, depth: 1, noise: 60, traits: 0 },
  { name: 'Scouts', budget: 9, depth: 1, noise: 40, traits: 0 },
  { name: 'Garrison', budget: 13, depth: 2, noise: 40, traits: 0 },
  { name: 'The Warden', budget: 18, depth: 2, noise: 20, traits: 1, boss: true },
  { name: 'Cavalry', budget: 23, depth: 2, noise: 20, traits: 0 },
  { name: 'Royal Guard', budget: 28, depth: 2, noise: 10, traits: 0 },
  { name: 'Vanguard', budget: 33, depth: 3, noise: 10, traits: 0 },
  { name: 'The Black King', budget: 39, depth: 3, noise: 0, traits: 2, boss: true },
];

export const UPGRADES = [
  { id: 'pawn', name: 'Militia', text: 'You start each run with one more pawn for each level.', costs: [3, 5, 8] },
  { id: 'gold', name: 'Treasury', text: 'You start each run with 5 more gold for each level.', costs: [2, 4, 6] },
  { id: 'bishop', name: 'Chaplain', text: 'You start each run with a bishop.', costs: [6] },
  { id: 'haggle', name: 'Haggler', text: 'Shop prices decrease by 10% for each level.', costs: [5, 8] },
];

const shuffle = (list) => {
  const a = [...list];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
};

// pool: [{ item, weight }]. Returns up to n different items.
function pickWeighted(pool, n) {
  const rest = [...pool], picked = [];
  while (picked.length < n && rest.length) {
    let roll = Math.random() * rest.reduce((sum, e) => sum + e.weight, 0);
    const i = rest.findIndex((e) => (roll -= e.weight) < 0);
    picked.push(rest.splice(i < 0 ? rest.length - 1 : i, 1)[0].item);
  }
  return picked;
}

// Home squares on the first two ranks, from the center to the edge.
const BACK_HOMES = [3, 2, 5, 1, 6, 0, 7, 4];
const FRONT_HOMES = [12, 11, 13, 10, 14, 9, 15, 8];

// Returns a free home square for a new piece, or -1 if the first two ranks are full.
export function freeHome(army, type) {
  const order = type === 'p' ? [...FRONT_HOMES, ...BACK_HOMES] : [...BACK_HOMES, ...FRONT_HOMES];
  return order.find((s) => !army.some((a) => a.home === s)) ?? -1;
}

export function startingArmy(upgrades) {
  const army = [
    { type: 'k', home: 4 }, { type: 'r', home: 0 }, { type: 'n', home: 6 },
    { type: 'p', home: 10 }, { type: 'p', home: 11 }, { type: 'p', home: 12 }, { type: 'p', home: 13 },
  ];
  if (upgrades.bishop) army.push({ type: 'b', home: 2 });
  for (let i = 0; i < (upgrades.pawn ?? 0); i++) army.push({ type: 'p', home: freeHome(army, 'p') });
  return army.map((a, i) => ({ id: i + 1, ...a }));
}

export function generateEnemy(floor) {
  const spec = FLOORS[floor - 1];
  const caps = { p: 8, n: 2, b: 2, r: 2, q: floor >= 5 ? 1 : 0 };
  const weights = { p: 4, n: 2, b: 2, r: 1.5, q: 1 };
  const counts = { p: 0, n: 0, b: 0, r: 0, q: 0 };
  let budget = spec.budget;
  for (;;) {
    const pool = Object.keys(caps)
      .filter((t) => counts[t] < caps[t] && PIECE_VALUE[t] <= budget)
      .map((t) => ({ item: t, weight: weights[t] }));
    if (!pool.length) break;
    const [type] = pickWeighted(pool, 1);
    counts[type]++;
    budget -= PIECE_VALUE[type];
  }
  const squares = {
    r: shuffle([56, 63]), n: shuffle([57, 62]), b: shuffle([58, 61]), q: [59],
    p: [52, 51, 53, 50, 54, 49, 55, 48],
  };
  const pieces = [{ type: 'k', square: 60 }];
  for (const type of Object.keys(counts)) {
    for (let i = 0; i < counts[type]; i++) pieces.push({ type, square: squares[type][i] });
  }
  const traits = shuffle(RELICS.filter((r) => r.foe)).slice(0, spec.traits).map((r) => r.id);
  return { pieces, traits };
}

const piecePool = (floor) => [
  { item: { kind: 'piece', type: 'p' }, weight: 3 },
  { item: { kind: 'piece', type: 'n' }, weight: 3 },
  { item: { kind: 'piece', type: 'b' }, weight: 3 },
  { item: { kind: 'piece', type: 'r' }, weight: 1.5 },
  ...(floor >= 3 ? [{ item: { kind: 'piece', type: 'q' }, weight: 0.5 }] : []),
];

const freshRelics = (owned, n) =>
  shuffle(RELICS.filter((r) => !owned.includes(r.id))).slice(0, n).map((r) => ({ kind: 'relic', id: r.id }));

// The three free rewards after a battle. floor is the floor that comes next.
export function rollDraft(floor, owned) {
  return pickWeighted([
    ...piecePool(floor),
    ...freshRelics(owned, 2).map((item) => ({ item, weight: 2 })),
    { item: { kind: 'gold', amount: 10 + 2 * floor }, weight: 2 },
  ], 3);
}

export function rollShop(floor, owned) {
  return [...pickWeighted(piecePool(floor), 2), ...freshRelics(owned, 2)];
}

export function price(item, haggle = 0) {
  const base = item.kind === 'piece' ? PIECE_PRICE[item.type] : RELIC_PRICE;
  return Math.max(1, Math.round(base * (1 - 0.1 * haggle)));
}
