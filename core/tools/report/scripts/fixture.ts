import { generateFixture } from "../src/fixture";

const [out = "fixture.json", games = "40", seed = "1"] = process.argv.slice(2);
const file = generateFixture({ games: Number(games), seed: Number(seed) });
await Bun.write(out, JSON.stringify(file));
console.log(`${out}: ${file.cells.length} cells, ${file.cells.length * file.games} games`);
