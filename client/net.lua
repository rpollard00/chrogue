--[[
  The connection to the Rust core (core/PROTOCOL.md). The client sends one request at a time and keeps the others in a queue.
  The socket does not block: net.update reads and writes what is ready, one time in each frame, thus the window does not
  stop while the core thinks.

  net.start(options) starts the core, or connects to a running core (options.connect = 'HOST:PORT').
  With options.embed, the core is in the process of the game (core.lua), and the client uses no socket. The core answers
  a request in the call. Thus the window stops for 0.1 to 0.2 seconds while the core selects the enemy move.
  net.send(request) puts a request in the queue. net.onResponse(response, request) gets each response.
  net.onConnect() is called after each connection, also after a reconnection.
]]
local json = require('json')
-- Only a core on a socket needs these two modules. net.start loads them. The WebAssembly build has no `bit`.
local socket, bit

local net = {
  -- 'starting', 'connecting', 'auth' (the first line of the connection), 'connected', 'lost', or 'failed'.
  state = 'starting',
  -- A text for the player when the state is 'lost' or 'failed'.
  problem = nil,
  address = nil,
  queue = {},
  inflight = nil,
  attempts = 0,
  -- True when the core accepted the token of the connection. False with --no-auth and a core that does not check tokens.
  authenticated = false,
  stats = { sent = 0, received = 0, lastMs = 0, worstMs = 0, connects = 0, timeouts = 0 },
}

local START_LIMIT = 10
local RETRY_TIME = 1
-- The time that the client waits for a response before it closes the connection. enemy_move can take longer at a high level of the AI.
net.TIMEOUT = 5
net.ENEMY_TIMEOUT = 15

local options, proc, tcp, token
-- The core in the process of the game, or nil for a core on a socket.
local embedded
local buffer, outgoing, sentAt = '', nil, 0
local nextId, retryAt, startedAt, checkedAt = 1, 0, 0, 0

local function now() return love.timer.getTime() end

local function shell(s) return "'" .. tostring(s):gsub("'", "'\\''") .. "'" end

local function exists(path)
  local file = io.open(path, 'rb')
  if file then file:close() end
  return file ~= nil
end

local function readFile(path)
  local file = io.open(path, 'rb')
  if not file then return nil end
  local text = file:read('*a')
  file:close()
  return text
end

-- A token of 64 hex digits for the connection to the core that the game starts. The bytes of /dev/urandom (when the game can
-- read it) and of a generator that has the time as its seed are mixed, thus the token is not empty when one source fails.
local function newToken()
  local rng = love.math.newRandomGenerator()
  rng:setSeed(os.time() % 4294967296, math.floor(love.timer.getTime() * 1e6) % 4294967296)
  local file = io.open('/dev/urandom', 'rb')
  local bytes = file and file:read(32) or ''
  if file then file:close() end
  local digits = {}
  for i = 1, 32 do
    digits[i] = ('%02x'):format(bit.bxor(bytes:byte(i) or 0, rng:random(0, 255)))
  end
  return table.concat(digits)
end

local function basename(path) return tostring(path):match('([^/]+)$') or tostring(path) end

-- True if the process `pid` is the core that the game started. The game never stops a process that is not its core: after
-- the core stops, the system can give its number to a different process.
local function isCore(pid)
  local name = basename(proc.path)
  local cmdline = readFile('/proc/' .. pid .. '/cmdline')
  if cmdline then return basename(cmdline:match('^[^%z]*')) == name end
  -- Linux has /proc for each process. No file there means that the process stopped.
  if exists('/proc/self/cmdline') then return false end
  -- macOS: ps gives the path of the program, or nothing if no process has this number.
  local pipe = io.popen('ps -p ' .. tonumber(pid) .. ' -o comm= 2>/dev/null')
  if not pipe then return false end
  local comm = (pipe:read('*a') or ''):gsub('%s+$', '')
  pipe:close()
  return comm ~= '' and basename(comm) == name
end

-- The core binary: CHROGUE_CORE, else ../core/target/release/chrogue-core from the game folder.
function net.findCore()
  local env = os.getenv('CHROGUE_CORE')
  if env and env ~= '' then return env, exists(env) end
  local source = love.filesystem.getSource():gsub('/+$', '')
  local path = source .. '/../core/target/release/chrogue-core'
  return path, exists(path)
end

local function alive() return proc ~= nil and isCore(proc.pid) end

local function fail(text)
  if tcp then tcp:close() end
  tcp, buffer, outgoing = nil, '', nil
  net.inflight = nil
  net.state, net.problem = 'failed', text
end

local function spawn()
  local core, found = net.findCore()
  if not found then
    return fail(('The game did not find the core at %s. Build it with "cargo build --release" in core/, or set CHROGUE_CORE.'):format(core))
  end
  love.filesystem.write('core-out.txt', '')
  love.filesystem.write('core-pid.txt', '')
  local dir = love.filesystem.getSaveDirectory()
  local args = { '--listen', '127.0.0.1:0' }
  if options.noSave then args[#args + 1] = '--no-save'
  else
    args[#args + 1] = '--save-dir'
    args[#args + 1] = options.saveDir or dir
  end
  if options.seed then args[#args + 1] = '--seed'; args[#args + 1] = tostring(options.seed) end
  if options.debug then args[#args + 1] = '--debug' end
  if options.keepAlive then args[#args + 1] = '--keep-alive' end
  for i, a in ipairs(args) do args[i] = shell(a) end
  local out, log, pidFile = dir .. '/core-out.txt', dir .. '/core-log.txt', dir .. '/core-pid.txt'
  -- The token goes to the core only in its environment (CHROGUE_TOKEN). The shell reads it from its input, thus it is not
  -- in the command line of a process, where other users can read it. The core runs in the background. Its first line
  -- goes to a file, thus the game does not wait for it.
  token = token or newToken()
  local command = table.concat({
    'IFS= read -r CHROGUE_TOKEN',
    'export CHROGUE_TOKEN',
    ('%s %s < /dev/null > %s 2> %s &'):format(shell(core), table.concat(args, ' '), shell(out), shell(log)),
    'echo $! > ' .. shell(pidFile),
  }, '\n')
  local pipe = io.popen(command, 'w')
  if not pipe then return fail('The core did not start.') end
  pipe:write(token, '\n')
  pipe:close()
  local pid = tonumber((readFile(pidFile) or ''):match('%d+'))
  if not pid then return fail('The core did not start.') end
  proc = { pid = pid, out = out, log = log, path = core }
  net.state, startedAt = 'starting', now()
end

local function lose(reason)
  if tcp then tcp:close() end
  tcp, buffer, outgoing = nil, '', nil
  net.inflight, net.queue = nil, {}
  net.state, net.problem = 'lost', reason
  net.authenticated = false
  retryAt = now() + RETRY_TIME
end

-- The connection can take commands.
local function ready(authenticated)
  net.state, net.problem, net.authenticated = 'connected', nil, authenticated
  net.stats.connects = net.stats.connects + 1
  if net.onConnect then net.onConnect() end
end

-- The TCP connection is open. The first line is the token: {"auth":"..."}. The core answers {"ok":true,"auth":true}
-- before it takes a command.
local function opened()
  if token then
    net.state = 'auth'
    outgoing = { text = json.encode({ auth = token }) .. '\n', at = 1 }
    sentAt = now()
  elseif options.noAuth then
    ready(false)
  else
    fail('The game has no token for the core. Set CHROGUE_TOKEN to the token of the core, or start the game with --no-auth.')
  end
end

local function connect(address)
  local host, port = address:match('^(.-):(%d+)$')
  if not host then return fail('The address must be HOST:PORT: ' .. tostring(address)) end
  net.address = address
  tcp = socket.tcp()
  tcp:settimeout(0)
  tcp:setoption('tcp-nodelay', true)
  local ok, err = tcp:connect(host, tonumber(port))
  if ok or err == 'already connected' then
    opened()
  elseif err == 'timeout' or err == 'Operation already in progress' then
    net.state = 'connecting'
  else
    net.state, net.problem = 'lost', 'The game cannot connect to the core: ' .. tostring(err)
    tcp:close()
    tcp = nil
    retryAt = now() + RETRY_TIME
  end
end

-- Opens the core in the process of the game. It has the same save folder and options as a core that the game starts.
local function embed()
  local dir = not options.noSave and (options.saveDir or love.filesystem.getSaveDirectory()) or nil
  local core, reason = require('core').open({
    save_dir = dir, seed = options.seed and ('%.0f'):format(options.seed), debug = options.debug or false,
  })
  if not core then return fail('The core did not open. ' .. tostring(reason)) end
  embedded = core
  net.address = 'in process'
  ready(false)
end

function net.start(opts)
  options = opts
  if opts.embed then return embed() end
  socket, bit = require('socket'), require('bit')
  if opts.connect then
    -- A core that a different game started has its own token. The player gives it in CHROGUE_TOKEN.
    local env = os.getenv('CHROGUE_TOKEN')
    token = env ~= '' and env or nil
    connect(opts.connect)
  else
    spawn()
  end
end

-- Adds a request to the queue. The request is a table such as { cmd = 'move', from = 12, to = 28 }.
function net.send(request)
  net.queue[#net.queue + 1] = request
end

-- Puts a request before the other requests in the queue.
function net.sendFirst(request)
  table.insert(net.queue, 1, request)
end

-- True while a request waits for its response or in the queue.
function net.busy() return net.inflight ~= nil or #net.queue > 0 end

-- Reads what the socket has, and gives each full line to `handle`. `handle` returns false to keep the other lines.
local function receive(handle)
  local data, err, partial = tcp:receive('*a')
  local got = data or partial
  if got and #got > 0 then buffer = buffer .. got end
  while tcp do
    local line, rest = buffer:match('^([^\n]*)\n(.*)$')
    if not line then break end
    buffer = rest
    if #line > 0 then
      local ok, response = pcall(json.decode, line)
      if not ok then error('The core sent a line that is not JSON: ' .. tostring(response)) end
      if handle(response) == false then break end
    end
  end
  if err == 'closed' and tcp then lose('The connection to the core is lost.') end
end

local function response(r)
  local request = net.inflight
  net.inflight = nil
  local ms = (now() - sentAt) * 1000
  local stats = net.stats
  stats.received, stats.lastMs, stats.worstMs = stats.received + 1, ms, math.max(stats.worstMs, ms)
  if net.onResponse then net.onResponse(r, request, ms) end
  -- A response can open a new connection state (a quit, a lost connection). The other lines wait.
  return net.state == 'connected'
end

-- The answer to the token: the first line of the connection.
local function authResponse(r)
  if r.ok == true and r.auth == true then
    ready(true)
  elseif options.noAuth then
    -- A core that does not check tokens answers the line {"auth":...} with bad_request. --no-auth accepts this core.
    local code = type(r.error) == 'table' and tostring(r.error.code) or '?'
    print(('The core does not check the token (%s). The game continues with no token, as --no-auth permits.'):format(code))
    ready(false)
  else
    local message = type(r.error) == 'table' and tostring(r.error.message or r.error.code) or 'no reason'
    fail('The core did not accept the token of the game: ' .. message)
  end
  return false
end

local function flush()
  if not outgoing then return end
  local last, err, partial = tcp:send(outgoing.text, outgoing.at)
  local sent = last or partial
  if sent then outgoing.at = sent + 1 end
  if outgoing.at > #outgoing.text then outgoing = nil
  elseif err and err ~= 'timeout' then lose('The connection to the core is lost.') end
end

-- Takes the next request of the queue. The request gets its id, and it waits for its response.
local function take()
  local request = table.remove(net.queue, 1)
  request.id = nextId
  nextId = nextId + 1
  net.inflight = request
  sentAt = now()
  net.stats.sent = net.stats.sent + 1
  return request
end

local function write()
  if not outgoing and not net.inflight and #net.queue > 0 then
    outgoing = { text = json.encode(take()) .. '\n', at = 1 }
  end
  flush()
end

-- Gives the next request of the queue to the core in the process. The response comes in the same call.
local function call()
  if net.inflight or #net.queue == 0 then return end
  local line = embedded:command(json.encode(take()))
  local ok, decoded = pcall(json.decode, line)
  if not ok then error('The core gave a line that is not JSON: ' .. tostring(decoded)) end
  -- Only a fault inside the core gives a response with no view. The game cannot continue with this core.
  if not decoded.view then
    return fail('The core failed. ' .. tostring(type(decoded.error) == 'table' and decoded.error.message or ''))
  end
  response(decoded)
end

-- The time that a request can wait for its response.
local function limitOf(request)
  return request and request.cmd == 'enemy_move' and net.ENEMY_TIMEOUT or net.TIMEOUT
end

local function timedOut(limit)
  net.stats.timeouts = net.stats.timeouts + 1
  lose(('The core did not answer in %d seconds.'):format(limit))
end

function net.update()
  local state = net.state
  if embedded then
    -- One request in each frame, thus the frame shows each response before the next request.
    if state == 'connected' then call() end
  elseif state == 'starting' then
    local first = (readFile(proc.out) or ''):match('^([^\n]*)\n')
    local address = first and first:match('"listening"%s*:%s*"([^"]+)"')
    if address then return connect(address) end
    if now() - startedAt > START_LIMIT or (now() - checkedAt > 0.5 and not alive()) then
      checkedAt = now()
      if not alive() or now() - startedAt > START_LIMIT then
        local log = (readFile(proc.log) or ''):gsub('%s+$', '')
        fail('The core did not start. ' .. (log ~= '' and log or ''))
      end
    end
  elseif state == 'connecting' then
    local _, writable = socket.select(nil, { tcp }, 0)
    if writable and #writable > 0 then
      if tcp:getpeername() then opened() else lose('The game cannot connect to the core.') end
    end
  elseif state == 'auth' then
    flush()
    if net.state == 'auth' then receive(authResponse) end
    if net.state == 'auth' and now() - sentAt > net.TIMEOUT then timedOut(net.TIMEOUT) end
  elseif state == 'connected' then
    write()
    if net.state == 'connected' then receive(response) end
    if net.state == 'connected' and net.inflight and now() - sentAt > limitOf(net.inflight) then timedOut(limitOf(net.inflight)) end
  elseif state == 'lost' and now() >= retryAt then
    net.attempts = net.attempts + 1
    -- The core that the game started stops when it has no client for some seconds. Then the game starts a new core.
    if proc and not alive() then spawn() else connect(net.address) end
  end
end

-- Closes the connection. The core that the game started also stops, unless the player asked to keep it (--keep-alive).
function net.shutdown()
  if embedded then
    embedded:close()
    embedded = nil
    net.state = 'failed'
    return
  end
  local own = proc and not options.keepAlive
  if tcp then
    if own and net.state == 'connected' then
      tcp:settimeout(0.2)
      tcp:send('{"cmd":"quit"}\n')
    end
    tcp:close()
    tcp = nil
  end
  if own and alive() then
    -- The core stops after quit. If it did not get the request, the game stops it. alive() checks that the number is
    -- still the number of the core, thus the game does not stop a different process.
    local limit = now() + 0.5
    while alive() and now() < limit do socket.sleep(0.02) end
    if alive() then os.execute('kill ' .. tonumber(proc.pid) .. ' 2>/dev/null') end
  elseif proc and options.keepAlive and net.address then
    print(('The core continues at %s. To play on, start the game with CHROGUE_TOKEN=%s and --connect %s.'):format(net.address, token or '', net.address))
  end
  net.state = 'failed'
end

function net.pid() return proc and proc.pid end

-- True if `pid` is the core that this game started (for the test of the shutdown).
function net.isCore(pid) return proc ~= nil and isCore(pid) end

return net
