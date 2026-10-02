--[[
  The connection to the Rust core (core/PROTOCOL.md). The client sends one request at a time and keeps the others in a queue.
  The socket does not block: net.update reads and writes what is ready, one time in each frame, thus the window does not
  stop while the core thinks.

  net.start(options) starts the core, or connects to a running core (options.connect = 'HOST:PORT').
  net.send(request) puts a request in the queue. net.onResponse(response, request) gets each response.
  net.onConnect() is called after each connection, also after a reconnection.
]]
local json = require('json')
local socket = require('socket')

local net = {
  -- 'starting', 'connecting', 'connected', 'lost', or 'failed'.
  state = 'starting',
  -- A text for the player when the state is 'lost' or 'failed'.
  problem = nil,
  address = nil,
  queue = {},
  inflight = nil,
  attempts = 0,
  stats = { sent = 0, received = 0, lastMs = 0, worstMs = 0, connects = 0 },
}

local START_LIMIT = 10
local RETRY_TIME = 1

local options, proc, tcp
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

-- The core binary: CHROGUE_CORE, else ../../core/target/release/chrogue-core from the game folder.
function net.findCore()
  local env = os.getenv('CHROGUE_CORE')
  if env and env ~= '' then return env, exists(env) end
  local source = love.filesystem.getSource():gsub('/+$', '')
  local path = source .. '/../../core/target/release/chrogue-core'
  return path, exists(path)
end

local function alive()
  if not proc then return false end
  local status = os.execute('kill -0 ' .. proc.pid .. ' 2>/dev/null')
  return status == 0 or status == true
end

local function fail(text)
  net.state, net.problem = 'failed', text
end

local function spawn()
  local core, found = net.findCore()
  if not found then
    return fail(('The game did not find the core at %s. Build it with "cargo build --release" in core/, or set CHROGUE_CORE.'):format(core))
  end
  love.filesystem.write('core-out.txt', '')
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
  local out, log = dir .. '/core-out.txt', dir .. '/core-log.txt'
  -- The core runs in the background. Its first line goes to a file, thus the game does not wait for it.
  local command = ('%s %s > %s 2> %s & echo $!'):format(shell(core), table.concat(args, ' '), shell(out), shell(log))
  local pipe = assert(io.popen(command))
  local pid = tonumber(pipe:read('*l'))
  pipe:close()
  if not pid then return fail('The core did not start.') end
  proc = { pid = pid, out = out, log = log, path = core }
  net.state, startedAt = 'starting', now()
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
    net.state = 'connected'
  elseif err == 'timeout' or err == 'Operation already in progress' then
    net.state = 'connecting'
  else
    net.state, net.problem = 'lost', 'The game cannot connect to the core: ' .. tostring(err)
    tcp:close()
    tcp = nil
    retryAt = now() + RETRY_TIME
    return
  end
  if net.state == 'connected' then
    net.stats.connects = net.stats.connects + 1
    net.problem = nil
    if net.onConnect then net.onConnect() end
  end
end

function net.start(opts)
  options = opts
  if opts.connect then connect(opts.connect) else spawn() end
end

local function lose(reason)
  if tcp then tcp:close() end
  tcp, buffer, outgoing = nil, '', nil
  net.inflight, net.queue = nil, {}
  net.state, net.problem = 'lost', reason
  retryAt = now() + RETRY_TIME
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

local function receive()
  local data, err, partial = tcp:receive('*a')
  local got = data or partial
  if got and #got > 0 then buffer = buffer .. got end
  while true do
    local line, rest = buffer:match('^([^\n]*)\n(.*)$')
    if not line then break end
    buffer = rest
    if #line > 0 then
      local ok, response = pcall(json.decode, line)
      if not ok then error('The core sent a line that is not JSON: ' .. tostring(response)) end
      local request = net.inflight
      net.inflight = nil
      local ms = (now() - sentAt) * 1000
      local stats = net.stats
      stats.received, stats.lastMs, stats.worstMs = stats.received + 1, ms, math.max(stats.worstMs, ms)
      if net.onResponse then net.onResponse(response, request, ms) end
    end
  end
  if err == 'closed' then lose('The connection to the core is lost.') end
end

local function write()
  if not outgoing and not net.inflight and #net.queue > 0 then
    local request = table.remove(net.queue, 1)
    request.id = nextId
    nextId = nextId + 1
    net.inflight = request
    outgoing = { text = json.encode(request) .. '\n', at = 1 }
    sentAt = now()
    net.stats.sent = net.stats.sent + 1
  end
  if outgoing then
    local last, err, partial = tcp:send(outgoing.text, outgoing.at)
    local sent = last or partial
    if sent then outgoing.at = sent + 1 end
    if outgoing.at > #outgoing.text then outgoing = nil
    elseif err and err ~= 'timeout' then lose('The connection to the core is lost.') end
  end
end

function net.update()
  local state = net.state
  if state == 'starting' then
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
      if tcp:getpeername() then
        net.state, net.problem = 'connected', nil
        net.stats.connects = net.stats.connects + 1
        if net.onConnect then net.onConnect() end
      else
        lose('The game cannot connect to the core.')
      end
    end
  elseif state == 'connected' then
    write()
    if net.state == 'connected' then receive() end
  elseif state == 'lost' and now() >= retryAt then
    net.attempts = net.attempts + 1
    -- The core that the game started stops when it has no client for some seconds. Then the game starts a new core.
    if proc and not alive() then spawn() else connect(net.address) end
  end
end

-- Closes the connection. The core that the game started also stops, unless the player asked to keep it (--keep-alive).
function net.shutdown()
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
    -- The core stops after quit. If it did not get the request, the game stops it.
    local limit = now() + 0.5
    while alive() and now() < limit do socket.sleep(0.02) end
    if alive() then os.execute('kill ' .. proc.pid .. ' 2>/dev/null') end
  elseif proc and options.keepAlive and net.address then
    print(('The core continues at %s. Start the game with --connect %s to play on.'):format(net.address, net.address))
  end
  net.state = 'failed'
end

function net.pid() return proc and proc.pid end

return net
