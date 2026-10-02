// A client of `chrogue-core --stdio` for the Bun scripts. One request, then one response line.
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

export const CORE_DIR = resolve(import.meta.dir, '..');
export const BIN = resolve(CORE_DIR, 'target/release/chrogue-core');

/** Builds the release binary if it is not there. */
export function ensureBinary(): void {
  if (existsSync(BIN)) return;
  const build = Bun.spawnSync(['cargo', 'build', '--release', '-p', 'chrogue-server'], { cwd: CORE_DIR, stdout: 'inherit', stderr: 'inherit' });
  if (build.exitCode !== 0) throw new Error('cargo build failed');
}

export type Json = any;

export class Core {
  private readonly proc: Bun.Subprocess<'pipe', 'pipe', 'inherit'>;
  private readonly reader: ReadableStreamDefaultReader<Uint8Array>;
  private readonly decoder = new TextDecoder();
  private buffer = '';
  private next = 1;
  /** Each response line, in order. */
  readonly transcript: string[] = [];

  constructor(args: string[]) {
    ensureBinary();
    this.proc = Bun.spawn([BIN, '--stdio', ...args], { stdin: 'pipe', stdout: 'pipe', stderr: 'inherit' });
    this.reader = this.proc.stdout.getReader();
  }

  private async line(): Promise<string> {
    for (;;) {
      const end = this.buffer.indexOf('\n');
      if (end >= 0) {
        const line = this.buffer.slice(0, end);
        this.buffer = this.buffer.slice(end + 1);
        return line;
      }
      const { value, done } = await this.reader.read();
      if (done) throw new Error('chrogue-core closed its output');
      this.buffer += this.decoder.decode(value, { stream: true });
    }
  }

  /** Sends a raw line and returns the parsed response. */
  async raw(text: string): Promise<Json> {
    this.proc.stdin.write(`${text}\n`);
    this.proc.stdin.flush();
    const line = await this.line();
    this.transcript.push(line);
    return JSON.parse(line);
  }

  /** Sends a command with a new id. Throws if the response does not echo the id. */
  async send(cmd: string, args: Record<string, unknown> = {}): Promise<Json> {
    const id = this.next++;
    if ("id" in args || "cmd" in args) throw new Error(`${cmd}: an argument cannot be named id or cmd`);
    const reply = await this.raw(JSON.stringify({ ...args, id, cmd }));
    if (reply.id !== id) throw new Error(`The response to ${cmd} has the id ${reply.id}, not ${id}`);
    return reply;
  }

  /** Sends a command that must succeed. */
  async ok(cmd: string, args: Record<string, unknown> = {}): Promise<Json> {
    const reply = await this.send(cmd, args);
    if (!reply.ok) throw new Error(`${cmd} ${JSON.stringify(args)} failed: ${JSON.stringify(reply.error)}`);
    return reply;
  }

  async close(): Promise<number> {
    this.proc.stdin.write('{"cmd":"quit"}\n');
    this.proc.stdin.end();
    return await this.proc.exited;
  }
}

// mulberry32, as in core/difftest/run.ts. The scripts do not use Math.random for their choices.
export function makeRng(seed: number) {
  let a = seed >>> 0;
  const next = (): number => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const int = (n: number): number => Math.floor(next() * n);
  const pick = <T>(list: readonly T[]): T => list[int(list.length)];
  const chance = (p: number): boolean => next() < p;
  const shuffle = <T>(list: readonly T[]): T[] => {
    const a = [...list];
    for (let i = a.length - 1; i > 0; i--) {
      const j = int(i + 1);
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  };
  return { next, int, pick, chance, shuffle };
}
export type Rng = ReturnType<typeof makeRng>;

export function option(name: string, fallback: number): number {
  const i = process.argv.indexOf(`--${name}`);
  if (i < 0) return fallback;
  const value = Number(process.argv[i + 1]);
  if (!Number.isInteger(value) || value < 0) throw new Error(`--${name} must be a whole number`);
  return value;
}
