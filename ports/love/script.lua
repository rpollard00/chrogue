--[[
  The test hook: `--script <file>` gives a Lua file that returns a list of steps. The steps drive the game with no person.
  A click goes through love.mousemoved, love.mousepressed and love.mousereleased, thus it uses the input path of a real click.

  { 'click', 'e2' } or { 'click', 12 }   a square of the battle board, or a home square of the camp (a1 to h2, 0 to 15)
  { 'press', 'continue' }                 a named control of the screen or of the dialog. See `control` in each screen module.
  { 'press', 'promo', 1 }                 a control with an argument: an item of the promotion picker, a card, a slot
  { 'hover', 'medal', 'player', 2 }       the pointer on a medal of a fan: 'player' or 'enemy', and the number of the medal
  { 'hover', 'stash', 'player' }          the pointer on a stash
  { 'hover', 'control', NAME, ARG }       the pointer on a named control
  { 'hover', 'none' }                     the pointer leaves the window
  { 'play', function(view) return move end }   clicks the two squares of a move that the function selects from view.moves.
                                          If the move has `promo`, the step selects that piece in the promotion picker.
  { 'again' }                             a click at the point of the last click, for a double click
  { 'down', NAME, ARG }, { 'up' }         a press of the button on a named control, and its release at the same point
  { 'refusal', 'enemy_move', 'wrong_phase', 2 }   the core can refuse this command with this code (2 times). Each
                                          refusal that the script does not expect stops the game with an error.
  { 'events', { { type = 'save_problem', ... } } }   events as in a response, for the events that the core cannot make
  { 'respond', function(app) return response, request end }   a response as from the core, for a state that it cannot make
  { 'key', 'f1' }                         a key
  { 'wait', 0.5 }                         seconds
  { 'settle' }                            until no request waits for the core, the enemy moved, and no piece moves
  { 'screen', 'camp' }                    until the screen is camp, and settled
  { 'send', { cmd = 'debug_set_floor', floor = 8 } }   a request to the core, to prepare a test (debug commands need --debug)
  { 'expect', function(view, client, app) return ok, detail end, 'label' }   stops the game with an error if not ok
  { 'size', 1920, 1080 }                  the size of the window
  { 'screenshot', '/absolute/path.png' }
  { 'dump', '/absolute/path.json' }       the last view of the core and the state of the client
  { 'fps', 3, 'camp' }                    counts the frames for 3 seconds and prints the frames per second with a label
  { 'log', 'text' }                       a line on stdout
  { 'quit' }

  A 'settle' or 'screen' step stops the game with an error after 30 seconds.
]]
local net = require('net')

local script = {}

local LIMIT = 30

local function center(r) return r.x + r.w / 2, r.y + r.h / 2 end

local function square(name)
  if type(name) == 'number' then return name end
  return ('abcdefgh'):find(name:sub(1, 1), 1, true) - 1 + 8 * (tonumber(name:sub(2, 2)) - 1)
end

-- The point of the stage for a step.
local function target(app, step)
  local kind, a, b, c = step[1], step[2], step[3], step[4]
  if kind == 'click' then
    return center(app.control(app.name == 'camp' and 'home' or 'square', square(a)))
  elseif kind == 'press' then
    return center(app.control(a, b))
  elseif kind == 'hover' then
    if a == 'medal' then return center(app.control('medal', { b, c })) end
    if a == 'stash' then return center(app.control('stash', b)) end
    if a == 'control' then return center(app.control(b, c)) end
  end
  error('The script has a step that this module does not know: ' .. tostring(kind) .. ' ' .. tostring(a))
end

function script.load(path)
  local chunk = assert(loadfile(path))
  return { steps = chunk(), at = 1, waitUntil = nil, pending = false, since = nil, path = path, allowed = {}, last = nil }
end

-- True if the script expects that the core refuses `cmd` with `code`. The count of the refusal becomes one smaller.
function script.expected(self, cmd, code)
  for _, a in ipairs(self.allowed) do
    if a.cmd == cmd and a.code == code and a.left > 0 then
      a.left = a.left - 1
      return true
    end
  end
  return false
end

local function click(x, y)
  love.mousemoved(x, y, 0, 0, false)
  love.mousepressed(x, y, 1, false, 1)
  love.mousereleased(x, y, 1, false, 1)
end

local function describe(step)
  local parts = {}
  for _, v in ipairs(step) do parts[#parts + 1] = type(v) == 'table' and (v.cmd or 'table') or tostring(v) end
  return table.concat(parts, ' ')
end

-- Does the steps that are ready. Call it one time for each frame.
function script.update(self, app)
  while self.at <= #self.steps do
    if self.pending then return end
    local step = self.steps[self.at]
    local kind = step[1]
    local now = love.timer.getTime()
    if kind == 'wait' then
      self.waitUntil = self.waitUntil or now + step[2]
      if now < self.waitUntil then return end
      self.waitUntil = nil
    elseif kind == 'settle' or kind == 'screen' then
      self.since = self.since or now
      local ready = app.settled() and (kind == 'settle' or app.name == step[2])
      if not ready then
        if now - self.since > LIMIT then
          error(('Step %d (%s) did not end in %d seconds. The screen is %s.'):format(self.at, describe(step), LIMIT, tostring(app.name)))
        end
        return
      end
      self.since = nil
    elseif kind == 'click' or kind == 'press' then
      local x, y = app.toWindow(target(app, step))
      self.last = { x, y }
      click(x, y)
    elseif kind == 'again' then
      local p = assert(self.last, 'The step again needs a click before it')
      click(p[1], p[2])
    elseif kind == 'down' then
      local x, y = app.toWindow(target(app, { 'press', step[2], step[3] }))
      self.last = { x, y }
      love.mousemoved(x, y, 0, 0, false)
      love.mousepressed(x, y, 1, false, 1)
    elseif kind == 'up' then
      local p = assert(self.last, 'The step up needs a step down before it')
      love.mousereleased(p[1], p[2], 1, false, 1)
    elseif kind == 'refusal' then
      self.allowed[#self.allowed + 1] = { cmd = step[2], code = step[3], left = step[4] or 1 }
    elseif kind == 'events' then
      app.events(step[2])
    elseif kind == 'respond' then
      -- A response that the core cannot make in this state: the function gets the app and gives the response and the request.
      local response, request = step[2](app)
      net.onResponse(response, request)
    elseif kind == 'play' then
      -- The function selects a move of view.moves. The step clicks its two squares, and the first piece of the picker.
      -- After the end of the battle, the step does nothing.
      local move = app.view.phase == 'player' and step[2](app.view)
      if app.view.phase == 'player' and not move then error(('Step %d: the play function gave no move'):format(self.at)) end
      for _, s in ipairs(move and { move.from, move.to } or {}) do click(app.toWindow(center(app.control('square', s)))) end
      if app.screen.promotion then click(app.toWindow(center(app.control('promo', move.promo or 1)))) end
    elseif kind == 'hover' then
      if step[2] == 'none' then love.mousefocus(false)
      else
        local x, y = app.toWindow(target(app, step))
        love.mousemoved(x, y, 0, 0, false)
      end
    elseif kind == 'key' then
      love.keypressed(step[2], step[2], false)
    elseif kind == 'send' then
      net.send(step[2])
    elseif kind == 'expect' then
      local client = app.screen and app.screen:state() or {}
      local ok, detail = step[2](app.view, client, app)
      if not ok then error(('Step %d: expected %s. %s'):format(self.at, tostring(step[3]), tostring(detail or ''))) end
      print(('ok  %s'):format(tostring(step[3])))
    elseif kind == 'size' then
      love.window.setMode(step[2], step[3], { resizable = true, minwidth = 640, minheight = 360, vsync = app.options.novsync and 0 or 1 })
      love.resize(step[2], step[3])
    elseif kind == 'screenshot' then
      self.pending = true
      love.graphics.captureScreenshot(function(image)
        local file = assert(io.open(step[2], 'wb'))
        file:write(image:encode('png'):getString())
        file:close()
        self.pending = false
      end)
    elseif kind == 'dump' then
      local file = assert(io.open(step[2], 'w'))
      file:write(app.dump())
      file:close()
    elseif kind == 'fps' then
      -- Counts the frames for some seconds, and prints the frames per second and the work of a frame (update and draw).
      local f = app.frames
      if not self.measure then
        self.measure = { at = now, count = f.count, work = f.workTotal or 0, worst = 0 }
        f.worstWorkMs = 0
      end
      if now - self.measure.at < step[2] then return end
      local m = self.measure
      local frames = f.count - m.count
      local line = ('fps %s: %.1f frames per second, %.2f ms of work for each frame (longest %.1f ms), %s'):format(step[3],
        frames / (now - m.at), ((f.workTotal or 0) - m.work) / math.max(1, frames), f.worstWorkMs or 0,
        require('shaders').current().label)
      print(line)
      app.measurements = app.measurements or {}
      app.measurements[#app.measurements + 1] = line
      self.measure = nil
    elseif kind == 'log' then
      print(step[2])
    elseif kind == 'quit' then
      love.event.quit(0)
      self.at = #self.steps
    else
      error('The script has a step that this module does not know: ' .. tostring(kind))
    end
    self.at = self.at + 1
    -- A step that changes the picture ends the frame, thus the next screenshot shows the change.
    if kind ~= 'wait' and kind ~= 'settle' and kind ~= 'screen' and kind ~= 'expect' and kind ~= 'log' and kind ~= 'dump'
      and kind ~= 'refusal' then return end
  end
end

return script
