--[[
  Chrogue in LÖVE: a client of the Rust core (core/PROTOCOL.md). The core has all the rules. This client draws the view
  of each response, plays its events as motion, and sends the commands of the player.
  This file has the window, the frame, the command line, the screen manager, and the input. README.md tells how to run it.
]]
local gfx = require('gfx')
local json = require('json')
local layout = require('layout')
local net = require('net')
local script = require('script')
local shaders = require('shaders')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local lg = love.graphics
local C, px = theme.color, gfx.px

local SCREENS = {
  title = require('title'),
  upgrades = require('upgrades'),
  battle = require('battle'),
  camp = require('camp'),
  over = require('over'),
}

local app = {
  -- The current screen: an object of a module of SCREENS, and its name (the `screen` of the view).
  screen = nil, name = nil, view = nil,
  -- The question of the dialog that is open, or nil.
  dialog = nil,
  pointer = { x = nil, y = nil, hover = nil, pressed = nil },
  options = {},
  enteredAt = 0,
  -- The responses that the core refused, for the dump.
  refusals = {},
  frames = { count = 0, worstMs = 0, started = 0 },
}

local runner
local originX, originY = 0, 0
local TRANSITION = 0.2

function app.send(request) net.send(request) end

-- True while a request waits for the core. A screen does not send a second action before the first one is done.
function app.waiting() return net.busy() end

function app.confirm(question, ok) app.dialog = { question = question, ok = ok } end

-- Opens the screen of a view.
local function enter(view, events)
  local module = assert(SCREENS[view.screen], 'The core sent a screen that the client does not know: ' .. tostring(view.screen))
  app.screen = module.new(app, view, events)
  app.name = view.screen
  app.dialog = nil
  app.enteredAt = love.timer.getTime()
end

local function hasScreenEvent(events)
  for _, e in ipairs(events) do if e.type == 'screen' then return true end end
  return false
end

net.onConnect = function()
  -- After each connection, the client asks for the view. The core keeps the session, thus the same screen continues.
  net.sendFirst({ cmd = 'view' })
  app.resync = true
end

net.onResponse = function(response, request)
  local view = response.view
  app.view = view
  app.response = response
  if not view then return end
  local events = response.events or {}
  if not response.ok then
    local err = response.error or {}
    io.stderr:write(('The core refused %s: %s (%s)\n'):format(request and request.cmd or '?', tostring(err.code), tostring(err.message)))
    app.refusals[#app.refusals + 1] = { cmd = request and request.cmd, code = err.code }
    if app.screen and app.name == view.screen then app.screen:refused(view, request) else enter(view, {}) end
    return
  end
  if request and request.cmd == 'view' and app.resync then
    app.resync = false
    enter(view, events)
  elseif not app.screen or view.screen ~= app.name or hasScreenEvent(events) then
    enter(view, events)
  else
    app.screen:apply(view, events, request)
  end
end

-- The stage is 80 by 45 units. It becomes larger or smaller as one unit, and it stays in the center of the window.
local function fit()
  local w, h = lg.getDimensions()
  local u = math.min(w / layout.stage.w, h / layout.stage.h)
  if u ~= gfx.u then theme.dropFonts() end
  gfx.u = u
  originX, originY = math.floor((w - layout.stage.w * u) / 2), math.floor((h - layout.stage.h * u) / 2)
  shaders.resize()
end

local function toStage(x, y)
  x, y = shaders.toScene(x, y)
  return (x - originX) / gfx.u, (y - originY) / gfx.u
end

function app.toWindow(x, y) return shaders.toWindow(x * gfx.u + originX, y * gfx.u + originY) end

-- The control at a point of the stage. The dialog is above the screen.
function app.hit(x, y)
  if not x or net.state ~= 'connected' or not app.screen then return nil end
  if app.dialog then
    local d = ui.dialogLayout(layout.stage)
    if layout.contains(d.ok, x, y) then return 'ok' end
    if layout.contains(d.cancel, x, y) then return 'cancel' end
    return nil
  end
  return app.screen:hit(x, y)
end

-- The rectangle of a named control, for the test script.
function app.control(name, arg)
  if app.dialog then
    local d = ui.dialogLayout(layout.stage)
    return assert(d[name], 'The dialog has no control ' .. tostring(name))
  end
  local r = app.screen and app.screen:control(name, arg)
  return assert(r, ('The screen %s has no control %s %s'):format(tostring(app.name), tostring(name), tostring(arg)))
end

local function activate(name)
  if app.dialog then
    local dialog = app.dialog
    app.dialog = nil
    if name == 'ok' then dialog.ok() end
    return
  end
  app.screen:activate(name)
end

local function parse(args)
  local options = app.options
  local i = 1
  local function value()
    i = i + 1
    return args[i]
  end
  while i <= #args do
    local name = args[i]
    if name == '--size' then
      local w, h = tostring(value()):match('^(%d+)x(%d+)$')
      options.width, options.height = tonumber(w), tonumber(h)
    elseif name == '--script' then options.script = value()
    elseif name == '--seed' then options.seed = tonumber(value())
    elseif name == '--connect' then options.connect = value()
    elseif name == '--save-dir' then options.saveDir = value()
    elseif name == '--debug' then options.debug = true
    elseif name == '--no-save' then options.noSave = true
    elseif name == '--keep-alive' then options.keepAlive = true
    elseif name == '--novsync' then options.novsync = true
    end
    i = i + 1
  end
end

function love.load(args)
  parse(args)
  local options = app.options
  if options.width or options.novsync then
    local w, h = options.width or lg.getWidth(), options.height or lg.getHeight()
    love.window.setMode(w, h, { resizable = true, minwidth = 640, minheight = 360, vsync = options.novsync and 0 or 1 })
  end
  shaders.load()
  fit()
  -- An error stops the core that the game started, then shows the error.
  local default = love.errorhandler or love.errhand
  love.errorhandler = function(message)
    pcall(net.shutdown)
    if runner then
      -- With a script, an error stops the game. It does not wait for a person.
      io.stderr:write('Error: ' .. debug.traceback(tostring(message), 2) .. '\n')
      return function() return 1 end
    end
    return default(message)
  end
  if options.script then runner = script.load(options.script) end
  net.start(options)
  app.frames.started = love.timer.getTime()
end

function love.resize() fit() end

function love.quit()
  net.shutdown()
  return false
end

function love.update(dt)
  app.frames.workStart = love.timer.getTime()
  net.update()
  local frames = app.frames
  frames.count = frames.count + 1
  if frames.count > 5 then frames.worstMs = math.max(frames.worstMs, love.timer.getDelta() * 1000) end
  -- A long frame (a screenshot, a move of the window) does not skip the motion.
  if app.screen then app.screen:update(math.min(dt, 0.05), app.pointer) end
  if runner then script.update(runner, app) end
end

local function moved(x, y)
  local p = app.pointer
  p.x, p.y = x, y
  p.hover = app.hit(x, y)
end

function love.mousemoved(x, y)
  moved(toStage(x, y))
end

function love.mousepressed(x, y, button)
  if button ~= 1 then return end
  moved(toStage(x, y))
  app.pointer.pressed = app.pointer.hover
end

-- A click is a press and a release on the same control.
function love.mousereleased(x, y, button)
  if button ~= 1 then return end
  local p = app.pointer
  local pressed = p.pressed
  p.pressed = nil
  local sx, sy = toStage(x, y)
  moved(sx, sy)
  if pressed and pressed == p.hover then activate(pressed) end
  -- The click can change the controls, thus the control below the pointer can be a different one.
  moved(sx, sy)
end

function love.mousefocus(focus)
  if not focus then moved(nil, nil) end
end

function love.keypressed(key)
  if key == 'f1' then
    shaders.cycle()
    return
  end
  if app.dialog then
    if key == 'escape' then app.dialog = nil
    elseif key == 'return' or key == 'kpenter' then activate('ok') end
    return
  end
  if app.screen and net.state == 'connected' then app.screen:key(key) end
end

-- The panel of the connection: the core starts, the connection is lost, or the core is missing.
local function connection()
  local state = net.state
  if state == 'connected' and app.screen then return end
  local message = text.NET[state] or net.problem or ''
  if state == 'failed' then message = net.problem or 'The core stopped.' end
  local w = 30
  local lines = gfx.wrap(message, 'body', 1.05, w - 2.5)
  local extra = state == 'lost' and net.attempts > 0 and (text.NET.retry):format(net.attempts) or nil
  local h = 2.5 + #lines * 1.6 + (extra and 1.4 or 0)
  local x, y = (layout.stage.w - w) / 2, (layout.stage.h - h) / 2
  if app.screen then gfx.rect(0, 0, layout.stage.w, layout.stage.h, 0, C.black, 0.55) end
  gfx.shadow(x, y, w, h, px(10), px(10), px(40), 0, 0.6)
  gfx.rect(x, y, w, h, px(10), C.panel)
  gfx.outline(x, y, w, h, px(10), px(1), state == 'failed' and C.danger or C.line)
  gfx.lines(lines, 'body', 1.05, x + 1.25, y + 1.25, 1.6, { color = C.text })
  if extra then gfx.text(extra, 'body', 0.85, x + 1.25, y + 1.25 + #lines * 1.6, { color = C.dim, line = 1.4 }) end
end

function love.draw()
  shaders.beginScene(love.timer.getTime() - app.frames.started)
  lg.push()
  lg.translate(originX, originY)
  lg.scale(gfx.u)
  if app.screen then
    local pointer = app.pointer
    if app.dialog then pointer = { x = pointer.x, y = pointer.y } end
    app.screen:draw(pointer)
    -- A new screen comes into view from the background.
    local t = (love.timer.getTime() - app.enteredAt) / TRANSITION
    if t < 1 then gfx.rect(-1, -1, layout.stage.w + 2, layout.stage.h + 2, 0, C.bg, 1 - gfx.ease(t)) end
    if app.dialog then ui.dialog(layout.stage, app.dialog.question, app.pointer) end
  end
  connection()
  lg.pop()
  shaders.endScene()
  -- The name of the effects mode, in the corner of the window.
  local font = theme.font('body', 13)
  lg.setColor(0, 0, 0, 0.55)
  local label = shaders.current().label .. '  (F1)'
  lg.rectangle('fill', 6, lg.getHeight() - 26, font:getWidth(label) + 14, 20, 4, 4)
  lg.setColor(0.91, 0.90, 0.88, 0.9)
  lg.print(label, font, 13, lg.getHeight() - 24)
  -- The time of the work of the frame (update and draw), with no wait for the screen.
  local frames = app.frames
  if frames.workStart and frames.count > 5 then
    local ms = (love.timer.getTime() - frames.workStart) * 1000
    frames.workTotal = (frames.workTotal or 0) + ms
    frames.worstWorkMs = math.max(frames.worstWorkMs or 0, ms)
  end
end

-- The settled state, for the test script: no request waits, and the screen has no motion that changes its state.
function app.settled()
  return net.state == 'connected' and not net.busy() and app.screen ~= nil and app.screen:settled()
    and love.timer.getTime() - app.enteredAt > TRANSITION
end

-- The set areas of each screen, in stage units, for the test of the layout (test/layout.py).
local AREAS = {
  title = layout.title, upgrades = layout.upgrades, camp = layout.camp, over = layout.over,
  battle = { board = layout.board, foe = layout.foe, me = layout.me },
}

local function rects(t, prefix, list)
  list = list or {}
  if type(t) ~= 'table' then return list end
  if t.x and t.y and t.w and t.h then
    list[prefix] = { x = t.x, y = t.y, w = t.w, h = t.h }
    return list
  end
  for k, v in pairs(t) do rects(v, prefix == '' and tostring(k) or prefix .. '.' .. tostring(k), list) end
  return list
end

-- The last view of the core and the state of the client, as JSON.
function app.dump()
  local seconds = love.timer.getTime() - app.frames.started
  return json.encode({
    screen = app.name,
    view = app.view,
    client = app.screen and app.screen:state() or json.null,
    dialog = app.dialog and app.dialog.question or json.null,
    hover = app.pointer.hover or json.null,
    refusals = app.refusals,
    net = { state = net.state, address = net.address or json.null, stats = net.stats, pid = net.pid() or json.null },
    areas = app.name and rects(AREAS[app.name], '') or json.null,
    origin = { x = originX, y = originY },
    measurements = app.measurements or {},
    effects = shaders.current().label,
    window = { width = lg.getWidth(), height = lg.getHeight(), unit = gfx.u },
    fps = { now = love.timer.getFPS(), average = app.frames.count / math.max(seconds, 0.001), frames = app.frames.count,
      seconds = seconds, worstFrameMs = app.frames.worstMs, worstWorkMs = app.frames.worstWorkMs or 0,
      averageWorkMs = (app.frames.workTotal or 0) / math.max(1, app.frames.count - 5), fontsMade = theme.fontsMade },
  }, '  ') .. '\n'
end

_G.chrogue = app
return app
