import { dirname, join, resolve } from "node:path";
import { Glob } from "bun";
import { parseBalanceText } from "../src/analysis";

const root = resolve(import.meta.dir, "..");
const template = join(root, "dist", "index.html");
const stamp = join(root, "dist", ".sources");
const slot = '<script id="balance-data" type="application/json"></script>';

async function sourceHash(): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  const files = ["index.html", "vite.config.ts", "package.json", "bun.lock", ...[...new Glob("src/**/*").scanSync(root)].sort()];
  for (const file of files) hasher.update(file).update(await Bun.file(join(root, file)).bytes());
  return hasher.digest("hex");
}

async function build(): Promise<string> {
  const hash = await sourceHash();
  const cached = (await Bun.file(stamp).exists()) && (await Bun.file(stamp).text()) === hash && (await Bun.file(template).exists());
  if (!cached) {
    const vite = Bun.spawnSync(["bun", "x", "vite", "build"], { cwd: root, stdout: "inherit", stderr: "inherit" });
    if (vite.exitCode !== 0) process.exit(vite.exitCode);
    await Bun.write(stamp, hash);
  }
  return Bun.file(template).text();
}

function escapeForScript(json: string): string {
  return json.replaceAll("<", "\\u003c").replaceAll("\u2028", "\\u2028").replaceAll("\u2029", "\\u2029");
}

const [input, output] = process.argv.slice(2);
if (!input) {
  console.error("Usage: bun run report <balance.json> [out.html]");
  process.exit(2);
}
const json = await Bun.file(input).text();
const parsed = parseBalanceText(json);
if (parsed.kind === "error") {
  console.error(`${input}: ${parsed.message}`);
  process.exit(1);
}
const page = await build();
if (!page.includes(slot)) {
  console.error("The built page has no slot for the data.");
  process.exit(1);
}
const out = output ?? join(dirname(input), "balance.html");
const data = `<script id="balance-data" type="application/json">${escapeForScript(JSON.stringify(JSON.parse(json)))}</script>`;
await Bun.write(out, page.replace(slot, () => data));
const { dataset } = parsed;
console.log(`${out}: ${dataset.cells.length} cells, ${dataset.cells.length * dataset.games} games`);
