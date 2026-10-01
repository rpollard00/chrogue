export interface Weighted<T> {
  item: T;
  weight: number;
}

export function shuffle<T>(list: readonly T[]): T[] {
  const a = [...list];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// Returns up to n different items.
export function pickWeighted<T>(pool: readonly Weighted<T>[], n: number): T[] {
  const rest = [...pool], picked: T[] = [];
  while (picked.length < n && rest.length) {
    let roll = Math.random() * rest.reduce((sum, e) => sum + e.weight, 0);
    const i = rest.findIndex((e) => (roll -= e.weight) < 0);
    picked.push(...rest.splice(i < 0 ? rest.length - 1 : i, 1).map((e) => e.item));
  }
  return picked;
}
