/*
  Runs the test scripts of the client (ports/love/test) in the web build, in a Chromium with no window.
  Usage: bun ports/web/test/run.ts [run ...]. Build the game first: ports/web/build.sh.
  With no argument, the test does each run of RUNS. An argument is the name of a run, for example flow.

  The test makes a copy of ports/web/dist with the test scripts in game.love, serves it on a loopback port, and opens
  the page with ?script=test/NAME.lua. It reads the console of the page: a script ends with the line
  "The script is at its end.", and an error of the game is a line with "Error". The screenshots and the dumps of the
  scripts go to CHROGUE_OUT (default /tmp/chrogue-web/run), in one folder for each run.

  CHROME is the browser (default: chromium, google-chrome, or chromium-browser on the PATH). CHROME_FLAGS replaces the
  flags that make the browser draw with the graphics card. With software drawing, the game has 2 to 3 frames in each
  second, and the long scripts do not end in their time.
*/
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = import.meta.dir;
const dist = resolve(here, '../dist');
const out = resolve(process.env.CHROGUE_OUT ?? '/tmp/chrogue-web/run');
const GPU_FLAGS = '--use-angle=gl-egl --ignore-gpu-blocklist --enable-unsafe-swiftshader';
const END = 'The script is at its end.';
// A script writes its screenshots and dumps to this folder of the file system of the page.
const PAGE_OUT = '/tmp/chrogue-love4';
// The time that one run can take, and the time that the browser has to answer one command.
const RUN_LIMIT_MS = 240_000;
const COMMAND_LIMIT_MS = 30_000;
// After the end of a script, the game closes the core. The test reads the console for this time more.
const AFTER_END_MS = 500;
const WIDTH = 1440;
const HEIGHT = 900;

type Run = {
  name: string;
  // The options of the game, as the address gives them (ports/web/index.html).
  query: string;
  // The pixels of the screen for each CSS pixel.
  scale: number;
};

// The saved run needs two pages on one address: the second page finds the run that the first page saved.
const RUNS: Run[] = [
  { name: 'flow', query: 'seed=7&debug&no-save&script=test/flow.lua', scale: 1 },
  { name: 'flow-2x', query: 'seed=7&debug&no-save&script=test/flow.lua', scale: 2 },
  { name: 'battle', query: 'seed=7&debug&no-save&script=test/battle.lua', scale: 1 },
  { name: 'input', query: 'seed=7&debug&no-save&script=test/input.lua', scale: 1 },
  { name: 'floor8', query: 'seed=7&debug&no-save&script=test/floor8.lua', scale: 1 },
  { name: 'saved-a', query: 'seed=7&script=test/saved-a.lua', scale: 1 },
  { name: 'saved-b', query: 'seed=7&script=test/saved-b.lua', scale: 1 },
];

// The messages of the DevTools protocol that the test reads. The browser is the source, thus the test trusts the
// shapes of the methods that it names.
type Reply = { id: number; result?: unknown; error?: { message: string } };
type ConsoleCalled = { method: 'Runtime.consoleAPICalled'; params: { args: { value?: unknown; description?: string }[] } };
type ExceptionThrown = {
  method: 'Runtime.exceptionThrown';
  params: { exceptionDetails: { text: string; exception?: { description?: string } } };
};
type Event = ConsoleCalled | ExceptionThrown | { method: 'other' };

function isReply(message: object): message is Reply {
  return 'id' in message && typeof message.id === 'number';
}

// An event that the test reads, or `other`.
function toEvent(message: object): Event {
  if ('method' in message && (message.method === 'Runtime.consoleAPICalled' || message.method === 'Runtime.exceptionThrown')) {
    return message as ConsoleCalled | ExceptionThrown;
  }
  return { method: 'other' };
}

// The text field of the result of a command.
function field(result: unknown, name: string): string {
  const value = typeof result === 'object' && result !== null ? (result as Record<string, unknown>)[name] : undefined;
  if (typeof value !== 'string') throw new Error(`The browser gave no ${name}`);
  return value;
}

// The page makes the output folder of the scripts before the game starts. love.js sets the global Love.
const MAKE_OUTPUT_FOLDER = `Object.defineProperty(window, 'Love', { configurable: true, get() { return this.__love; }, set(start) {
  this.__love = (module) => {
    module.preRun = [...(module.preRun || []), () => module.FS.mkdirTree('${PAGE_OUT}')];
    return start(module);
  };
} });`;

// The files that the script of the page wrote, as base64 text for each name.
const READ_OUTPUT = `(() => {
  const files = {};
  for (const name of Module.FS.readdir('${PAGE_OUT}')) {
    if (name === '.' || name === '..') continue;
    const bytes = Module.FS.readFile('${PAGE_OUT}/' + name);
    let text = '';
    for (let i = 0; i < bytes.length; i += 8192) text += String.fromCharCode.apply(null, bytes.subarray(i, i + 8192));
    files[name] = btoa(text);
  }
  return JSON.stringify(files);
})()`;

const RENDERER = `(() => {
  const gl = document.createElement('canvas').getContext('webgl');
  const info = gl && gl.getExtension('WEBGL_debug_renderer_info');
  return String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : 'no WebGL');
})()`;

// The things that the test starts. `stop` removes each of them, also after an error. The last one goes first.
const started: (() => void | Promise<void>)[] = [];
async function stop() {
  for (const end of started.reverse()) await end();
  started.length = 0;
}

// The browser writes the address of its DevTools socket on stderr.
async function devtoolsAddress(stderr: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stderr.getReader();
  const decoder = new TextDecoder();
  let text = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) throw new Error(`The browser stopped before the test connected to it:\n${text}`);
    text += decoder.decode(value);
    const match = text.match(/DevTools listening on (ws:\/\/\S+)/);
    if (match) {
      reader.releaseLock();
      return match[1];
    }
  }
}

// The connection to one page of the browser.
type Page = {
  command: (method: string, params?: object) => Promise<unknown>;
  // The text that an expression gives in the page.
  evaluate: (expression: string) => Promise<string>;
  onEvent: (event: Event) => void;
};

async function openPage(address: string): Promise<Page> {
  const socket = new WebSocket(address);
  started.push(() => socket.close());
  await new Promise((ready, refused) => {
    socket.onopen = ready;
    socket.onerror = () => refused(new Error('The test cannot connect to the browser.'));
  });
  let nextId = 1;
  const waiting = new Map<number, { done: (result: unknown) => void; refuse: (error: Error) => void }>();
  const page: Page = { command: () => Promise.resolve(), evaluate: () => Promise.resolve(''), onEvent: () => {} };
  socket.onmessage = ({ data }) => {
    const message: unknown = JSON.parse(String(data));
    if (typeof message !== 'object' || message === null) return;
    if (!isReply(message)) return page.onEvent(toEvent(message));
    const request = waiting.get(message.id);
    waiting.delete(message.id);
    if (message.error) request?.refuse(new Error(message.error.message));
    else request?.done(message.result);
  };
  // A browser that stops does not answer. Each command that waits then fails, and no command waits with no end.
  socket.onclose = () => {
    for (const request of waiting.values()) request.refuse(new Error('The browser closed the connection.'));
    waiting.clear();
  };
  const send = (method: string, params: object, sessionId?: string): Promise<unknown> => {
    const id = nextId++;
    return new Promise((done, refuse) => {
      const timer = setTimeout(() => {
        waiting.delete(id);
        refuse(new Error(`The browser did not answer ${method} in ${COMMAND_LIMIT_MS / 1000} seconds.`));
      }, COMMAND_LIMIT_MS);
      const settle = <T>(end: (value: T) => void) => (value: T) => {
        clearTimeout(timer);
        end(value);
      };
      waiting.set(id, { done: settle(done), refuse: settle(refuse) });
      socket.send(JSON.stringify({ id, method, params, sessionId }));
    });
  };
  const targetId = field(await send('Target.createTarget', { url: 'about:blank' }), 'targetId');
  const sessionId = field(await send('Target.attachToTarget', { targetId, flatten: true }), 'sessionId');
  page.command = (method, params = {}) => send(method, params, sessionId);
  page.evaluate = async (expression) => {
    const reply = await page.command('Runtime.evaluate', { expression, returnByValue: true });
    const value = (reply as { result?: { value?: unknown } }).result?.value;
    return typeof value === 'string' ? value : '';
  };
  return page;
}

async function run(page: Page, address: string, test: Run): Promise<boolean> {
  const lines: string[] = [];
  let ended: (reason: 'end' | 'error' | 'time') => void = () => {};
  const end = new Promise<'end' | 'error' | 'time'>((resolve) => { ended = resolve; });
  await page.command('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: HEIGHT, deviceScaleFactor: test.scale, mobile: false });
  // A line of the page before this point is from the run before. A new page writes its lines after the browser
  // answers Page.navigate.
  page.onEvent = () => {};
  const navigated = page.command('Page.navigate', { url: `${address}/?${test.query}` });
  page.onEvent = (event) => {
    switch (event.method) {
      case 'Runtime.exceptionThrown': {
        const details = event.params.exceptionDetails;
        lines.push(`Error: ${details.exception?.description ?? details.text}`);
        ended('error');
        break;
      }
      case 'Runtime.consoleAPICalled': {
        const line = event.params.args.map((arg) => String(arg.value ?? arg.description ?? '')).join(' ');
        lines.push(line);
        if (line === END) ended('end');
        // The game writes this line when a script stops with an error (ports/love/main.lua).
        if (line.startsWith('Error:')) ended('error');
        break;
      }
      case 'other':
        break;
      default:
        event satisfies never;
    }
  };
  await navigated;
  const timer = setTimeout(() => ended('time'), RUN_LIMIT_MS);
  const reason = await end;
  clearTimeout(timer);
  // The traceback of an error, and a fault of the game after the end of its script, come after the line that ended
  // the run.
  await Bun.sleep(AFTER_END_MS);
  page.onEvent = () => {};

  // The run fails as a run of ports/love/test/run.sh does: an error of the game, or a refusal that the script did not
  // expect. A line that starts with "expected" is a refusal that the script expected.
  const bad = lines.filter((line) => /error|traceback|refused/i.test(line) && !line.startsWith('expected'));
  const ok = reason === 'end' && bad.length === 0;
  const folder = join(out, test.name);
  rmSync(folder, { recursive: true, force: true });
  mkdirSync(folder, { recursive: true });
  writeFileSync(join(folder, 'console.log'), lines.join('\n') + '\n');
  if (reason !== 'time') {
    const files: unknown = JSON.parse((await page.evaluate(READ_OUTPUT)) || '{}');
    for (const [name, data] of Object.entries(files ?? {})) {
      if (typeof data === 'string') writeFileSync(join(folder, name), Buffer.from(data, 'base64'));
    }
  }
  const checks = lines.filter((line) => line.startsWith('ok ')).length;
  console.log(`${ok ? 'ok ' : 'FAILED'}  ${test.name}: ${checks} checks${ok ? '' : `, stopped by ${reason}`}`);
  for (const line of lines) {
    if (/^(fps|enemy moves|expected)/.test(line)) console.log(`    ${line}`);
  }
  if (!ok) for (const line of reason === 'end' ? bad : lines.slice(-12)) console.log(`    ${line}`);
  return ok;
}

// The runs of the command line, or each run.
function selectRuns(names: string[]): Run[] {
  const unknown = names.filter((name) => !RUNS.some((test) => test.name === name));
  if (unknown.length > 0) throw new Error(`No run has the name ${unknown.join(', ')}.`);
  return names.length > 0 ? RUNS.filter((test) => names.includes(test.name)) : RUNS;
}

// Does the runs, and gives the number of runs that failed.
async function main(): Promise<number> {
  const selected = selectRuns(Bun.argv.slice(2));
  for (const file of ['index.html', 'love.js', 'love.wasm']) {
    if (!(await Bun.file(join(dist, file)).exists())) throw new Error(`${join(dist, file)} is missing. Run ports/web/build.sh first.`);
  }

  // The copy of the game with the test scripts. ports/web/dist stays the game for the players.
  const site = mkdtempSync(join(tmpdir(), 'chrogue-web-test-'));
  started.push(() => rmSync(site, { recursive: true, force: true }));
  for (const file of ['love.js', 'love.wasm']) await Bun.write(join(site, file), Bun.file(join(dist, file)));
  const packed = Bun.spawnSync([resolve(here, '../build.sh'), 'game'], {
    env: { ...process.env, CHROGUE_WEB_DIST: site, CHROGUE_WEB_TESTS: '1' },
    stdout: 'ignore',
    stderr: 'inherit',
  });
  if (packed.exitCode !== 0) throw new Error('build.sh game failed.');

  const server = Bun.serve({
    port: 0,
    hostname: '127.0.0.1',
    async fetch(request) {
      const path = new URL(request.url).pathname;
      const file = Bun.file(join(site, path === '/' ? 'index.html' : path));
      return (await file.exists()) ? new Response(file) : new Response('Not found', { status: 404 });
    },
  });
  started.push(() => server.stop(true));

  const chrome = process.env.CHROME ?? ['chromium', 'google-chrome', 'chromium-browser'].map((name) => Bun.which(name)).find(Boolean);
  if (!chrome) throw new Error('The test did not find Chromium. Set CHROME to the path of the browser.');
  const profile = mkdtempSync(join(tmpdir(), 'chrogue-web-profile-'));
  // The processes of the browser write to the folder until they stop.
  started.push(() => rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }));
  const browser = Bun.spawn(
    [chrome, '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, `--window-size=${WIDTH},${HEIGHT}`,
      '--no-first-run', '--no-default-browser-check', ...(process.env.CHROME_FLAGS ?? GPU_FLAGS).split(' ').filter(Boolean),
      'about:blank'],
    { stdout: 'ignore', stderr: 'pipe' },
  );
  started.push(async () => {
    browser.kill();
    await browser.exited;
  });

  const page = await openPage(await devtoolsAddress(browser.stderr));
  await page.command('Page.enable');
  await page.command('Runtime.enable');
  await page.command('Page.addScriptToEvaluateOnNewDocument', { source: MAKE_OUTPUT_FOLDER });
  console.log(`The browser draws with: ${await page.evaluate(RENDERER)}`);

  let failed = 0;
  for (const test of selected) {
    if (!(await run(page, `http://127.0.0.1:${server.port}`, test))) failed += 1;
  }
  console.log(failed > 0 ? `${failed} of ${selected.length} runs failed. See ${out}`
    : `No errors in ${selected.length} runs. The screenshots and the dumps are in ${out}`);
  return failed;
}

let failed = 1;
try {
  failed = await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
} finally {
  await stop();
}
process.exit(failed > 0 ? 1 : 0);
