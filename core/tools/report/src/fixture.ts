import { ENDS, RESULTS, type EnemyLevel, type Floor, type Level, type Relic } from "./analysis";

export interface BalanceFile {
  format: "chrogue-balance";
  version: 1;
  command: string;
  seed: number;
  games: number;
  max_plies: number;
  gold: number;
  content: { relics: Relic[]; levels: Level[]; floors: Floor[] };
  axes: {
    floors: number[];
    players: number[];
    enemies: EnemyLevel[];
    armies: string[];
    traits: string[];
    relics: string[][];
  };
  cells: {
    floor: number;
    player: number;
    enemy: number;
    army: number;
    traits: number;
    relics: number;
    results: string;
    ends: string;
    plies: number[];
    gold: number[];
    lost: number[];
    recruits: number;
  }[];
}

export interface FixtureOptions {
  seed: number;
  games: number;
  floors: number[];
  players: number[];
  enemies: EnemyLevel[];
  armies: string[];
  traits: string[];
  singleCount: number;
  pairedCount: number;
  tripleCount: number;
}

export const DEFAULT_FIXTURE: FixtureOptions = {
  seed: 1,
  games: 40,
  floors: [1, 2, 3, 4, 5, 6, 7, 8],
  players: [2, 3, 4, 6],
  enemies: ["floor", 5],
  armies: ["auto", "auto+3"],
  traits: ["floor", "none"],
  singleCount: 12,
  pairedCount: 5,
  tripleCount: 4,
};

export const PLANTED = {
  strong: "queenFlight",
  weak: "backpedal",
  synergy: ["forcedMarch", "longLeap"],
  antiSynergy: ["forcedMarch", "earlyPromo"],
  triple: ["longLeap", "earlyPromo", "queenFlight"],
  fade: 0.5,
} as const;

const relic = (key: string, name: string, text: string, kind: Relic["kind"], strength: number): Relic & { strength: number } => ({
  key,
  name,
  text,
  kind,
  trait: kind === "rule",
  strength,
});

const RELICS = [
  relic("forcedMarch", "Forced March", "Your pawns can always move two squares forward.", "rule", 0.15),
  relic("longLeap", "Long Leap", "Your knights can also jump three squares in one direction and one square to the side.", "rule", 0.25),
  relic("earlyPromo", "Field Promotion", "Your pawns promote one rank earlier.", "rule", 0.3),
  relic("queenFlight", "Queen's Flight", "Your queens can jump as a knight to an empty square.", "rule", 1.1),
  relic("backpedal", "Tactical Retreat", "Your pawns can move one square backward to an empty square.", "rule", -0.35),
  relic("kingKnight", "Royal Steed", "Your king can also move as a knight.", "rule", 0.2),
  relic("sidestep", "Sidestep", "Your bishops can move one square up, down, left, or right to an empty square.", "rule", 0.1),
  relic("bounty", "Bounty", "You get 50% more gold for each capture.", "effect", 0),
  relic("secondWind", "Second Wind", "The first piece that you lose in each battle returns after the battle.", "effect", 0),
  relic("conscription", "Conscription", "You start each battle with one more pawn. The pawn leaves after the battle.", "effect", 0.4),
  relic("interest", "Interest", "After each battle that you win, you get 1 gold for each 5 gold that you have. The maximum is 6 gold.", "effect", 0),
  relic("crossfire", "Crossfire", "Your rooks can capture a piece that is one square away diagonally.", "rule", 0.35),
  relic("vault", "Rampart Vault", "Your rooks can jump two squares up, down, left, or right to an empty square. A piece between does not stop the jump.", "rule", 0.05),
  relic("closeQuarters", "Close Quarters", "Your knights can capture a piece that is one square up, down, left, or right.", "rule", 0.3),
  relic("pilgrimLeap", "Pilgrim's Leap", "Your bishops can jump two squares diagonally. A piece between does not stop the jump.", "rule", 0.15),
];

const LEVEL_NAMES = ["Recruit", "Cadet", "Private", "Corporal", "Sergeant", "Lieutenant", "Captain", "Major", "Colonel", "General", "Marshal"];

const FLOORS: Floor[] = [
  { number: 1, name: "Border Patrol", level: 1, budget: 5, traits: 0, boss: false },
  { number: 2, name: "Scouts", level: 1, budget: 9, traits: 0, boss: false },
  { number: 3, name: "Garrison", level: 1, budget: 13, traits: 0, boss: false },
  { number: 4, name: "The Warden", level: 2, budget: 18, traits: 1, boss: true },
  { number: 5, name: "Cavalry", level: 2, budget: 23, traits: 0, boss: false },
  { number: 6, name: "Royal Guard", level: 2, budget: 28, traits: 0, boss: false },
  { number: 7, name: "Vanguard", level: 2, budget: 33, traits: 0, boss: false },
  { number: 8, name: "The Black King", level: 3, budget: 39, traits: 2, boss: true },
];

const SET_BONUS: Record<string, number> = {
  [[...PLANTED.synergy].sort().join("+")]: 1.2,
  [[...PLANTED.antiSynergy].sort().join("+")]: -0.5,
  [[...PLANTED.triple].sort().join("+")]: 1.5,
};

function mix(...parts: number[]): number {
  let h = 0x9e3779b9;
  for (const part of parts) {
    h = Math.imul(h ^ part, 0x85ebca6b);
    h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
    h ^= h >>> 16;
  }
  return h >>> 0;
}

function runSeed(seed: number, floor: number, army: number, game: number): number {
  return mix(seed, 77, floor, army, game);
}

function mulberry32(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function normal(random: () => number): number {
  return Math.sqrt(-2 * Math.log(1 - random())) * Math.cos(2 * Math.PI * random());
}

function armyBonus(spec: string): number {
  const match = /^auto([+-]\d+)$/.exec(spec);
  return match ? Number(match[1]) * 0.12 : 0;
}

function traitPenalty(spec: string, floor: Floor): number {
  if (spec === "none") return 0;
  if (spec === "floor") return floor.traits * 0.3;
  return spec.split("+").length * 0.3;
}

function loadoutStrength(keys: string[], playerLevel: number): number {
  const fade = 1 / (1 + PLANTED.fade * Math.max(0, playerLevel - 2));
  const parts = keys.reduce((sum, key) => sum + (RELICS.find((entry) => entry.key === key)?.strength ?? 0), 0);
  let bonus = 0;
  for (let mask = 1; mask < 1 << keys.length; mask++) {
    const subset = keys.filter((_, bit) => mask & (1 << bit));
    bonus += SET_BONUS[subset.sort().join("+")] ?? 0;
  }
  return (parts + bonus) * fade;
}

export function generateFixture(overrides: Partial<FixtureOptions> = {}): BalanceFile {
  const options = { ...DEFAULT_FIXTURE, ...overrides };
  const singles = RELICS.slice(0, options.singleCount).map((entry) => entry.key);
  const loadouts: string[][] = [[], ...singles.map((key) => [key])];
  for (let i = 0; i < options.pairedCount; i++) {
    for (let j = i + 1; j < options.pairedCount; j++) {
      const a = singles[i];
      const b = singles[j];
      if (a !== undefined && b !== undefined) loadouts.push([a, b]);
    }
  }
  for (let i = 0; i < options.tripleCount; i++) {
    for (let j = i + 1; j < options.tripleCount; j++) {
      for (let k = j + 1; k < options.tripleCount; k++) {
        const set = [singles[i], singles[j], singles[k]].filter((key) => key !== undefined);
        if (set.length === 3) loadouts.push(set);
      }
    }
  }
  const maxPlies = 300;
  const cells: BalanceFile["cells"] = [];
  const code = (list: readonly { code: string }[], index: number): string => list[index]?.code ?? "";

  options.floors.forEach((floorNumber, floor) => {
    const floorDef = FLOORS.find((entry) => entry.number === floorNumber) ?? FLOORS[0];
    if (!floorDef) return;
    options.players.forEach((playerLevel, player) => {
      options.enemies.forEach((enemySpec, enemy) => {
        const enemyLevel = enemySpec === "floor" ? floorDef.level : enemySpec;
        options.armies.forEach((armySpec, army) => {
          options.traits.forEach((traitSpec, traits) => {
            loadouts.forEach((keys, relics) => {
              const strength =
                0.45 * (playerLevel - enemyLevel) -
                0.08 * (floorDef.number - 1) +
                armyBonus(armySpec) -
                traitPenalty(traitSpec, floorDef) +
                loadoutStrength(keys, playerLevel);
              const random = mulberry32(mix(options.seed, floor, player, enemy, army, traits, relics));
              let results = "";
              let ends = "";
              const plies: number[] = [];
              const gold: number[] = [];
              const lost: number[] = [];
              for (let game = 0; game < options.games; game++) {
                const luck = normal(mulberry32(runSeed(options.seed, floor, army, game)));
                const z = strength + 1.3 * luck + 0.6 * normal(random);
                const result = z > 0.25 ? 0 : z < -0.25 ? 2 : 1;
                const roll = random();
                const end = result === 1 ? (roll < 0.4 ? 4 : roll < 0.7 ? 5 : 3) : roll < 0.7 ? 0 : roll < 0.92 ? 2 : 1;
                results += code(RESULTS, result);
                ends += code(ENDS, end);
                const length = end === 5 ? maxPlies : Math.round(30 + floorDef.budget * 1.5 + 60 * random() + (result === 0 ? 0 : 25));
                plies.push(Math.min(maxPlies, length));
                const captures = Math.round(floorDef.budget * (0.4 + 0.4 * random()));
                const reward = (keys.includes("bounty") ? 1.5 : 1) * captures + 3 + floorDef.number;
                gold.push(result === 0 ? Math.round(reward) : result === 1 ? Math.round(captures / 2) : 0);
                const damage = (result === 0 ? 0.15 : result === 1 ? 0.4 : 0.8) * floorDef.budget * (0.5 + random());
                lost.push(Math.max(0, Math.round(damage) - (keys.includes("secondWind") ? 2 : 0)));
              }
              cells.push({ floor, player, enemy, army, traits, relics, results, ends, plies, gold, lost, recruits: 0 });
            });
          });
        });
      });
    });
  });

  return {
    format: "chrogue-balance",
    version: 1,
    command: `fixture --seed ${options.seed} --games ${options.games}`,
    seed: options.seed,
    games: options.games,
    max_plies: maxPlies,
    gold: 0,
    content: {
      relics: RELICS.map(({ strength: _, ...entry }) => entry),
      levels: LEVEL_NAMES.map((name, i) => ({ number: i + 1, name })),
      floors: FLOORS,
    },
    axes: {
      floors: options.floors,
      players: options.players,
      enemies: options.enemies,
      armies: options.armies,
      traits: options.traits,
      relics: loadouts,
    },
    cells,
  };
}
