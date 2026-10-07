--[[
  Chrogue in LÖVE: a client of the Rust core (core/PROTOCOL.md). The core has all the rules. This client draws the view
  of each response, plays its events as motion, and sends the commands of the player.
  This file has the window, the frame, the command line, the screen manager, and the input. README.md tells how to run it.
]]
local debugmenu = require('debugmenu')
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
  relics = require('relics'),
  battle = require('battle'),
  camp = require('camp'),
  over = require('over'),
}

local app = {
  -- The current screen: an object of a module of SCREENS, and its name (the `screen` of the view).
  screen = nil, name = nil, view = nil,
  -- The question of the dialog that is open, or nil.
  dialog = nil,
  -- The debug menu (debugmenu.lua). It is above the screen and the dialog.
  debugMenu = nil,
  -- `pressed` is the control under the press of the button, and `pressedOn` the screen object of the press.
  pointer = { x = nil, y = nil, hover = nil, pressed = nil, pressedOn = nil },
  options = {},
  enteredAt = 0,
  -- The responses that the core refused, for the dump.
  refusals = {},
  -- The warnings about the saved data (save_failed, save_problem). They are above the layout and do not stop the game.
  notices = {},
  frames = { count = 0, worstMs = 0, started = 0 },
}

app.debugMenu = debugmenu.new(app)

local runner
local originX, originY = 0, 0
-- The pointer for a screen that is below the debug menu: no point and no control.
local NO_POINTER = {}
local TRANSITION = 0.2
local NOTICE_TIME = 14

-- The commands that the screens sent, for the test scripts: the last 64.
app.sent = {}

function app.send(request)
  app.sent[#app.sent + 1] = request.cmd
  if #app.sent > 64 then table.remove(app.sent, 1) end
  net.send(request)
end

-- True while a request waits for the core. A screen does not send a second action before the first one is done.
function app.waiting() return net.busy() end

function app.confirm(question, ok) app.dialog = { question = question, ok = ok } end

-- True for the name of a control of the debug menu, and for its chip.
local function ofMenu(name) return name ~= nil and name:find(debugmenu.PREFIX, 1, true) == 1 end

-- Opens the screen of a view. A press of the button on the screen before does not become a click on this screen. A
-- press on a control of the debug menu stays, because the menu is not a part of the screen.
local function enter(view, events)
  local module = assert(SCREENS[view.screen], 'The core sent a screen that the client does not know: ' .. tostring(view.screen))
  app.screen = module.new(app, view, events)
  app.name = view.screen
  app.dialog = nil
  app.enteredAt = love.timer.getTime()
  if not ofMenu(app.pointer.pressed) then app.pointer.pressed, app.pointer.pressedOn = nil, nil end
end

-- False during the fade of a new screen. A second click or key of the player is then for the screen before, thus the
-- new screen does not take it.
function app.inputReady() return love.timer.getTime() - app.enteredAt >= TRANSITION end

-- The events that are not for one screen: the problems of the saved data, and a relic that a feat unlocked.
function app.events(events)
  for _, e in ipairs(events) do
    local notice = text.notice(e, app.saveDir)
    if notice then
      notice.at = love.timer.getTime()
      notice.key = notice.title .. table.concat(notice.lines, ' ')
      -- Each save after a failed save can fail again. The same notice then stays longer, and does not come two times.
      local same
      for _, n in ipairs(app.notices) do if n.key == notice.key then same = n end end
      if same then same.at = math.max(same.at, notice.at - 0.2)
      else
        app.notices[#app.notices + 1] = notice
        io.stdout:write(('Notice: %s. %s\n'):format(notice.title, table.concat(notice.lines, ' ')))
      end
    end
  end
end

local function hasScreenEvent(events)
  for _, e in ipairs(events) do if e.type == 'screen' then return true end end
  return false
end

net.onConnect = function()
  -- After each connection, the client asks for the view. The core keeps the session, thus the same screen continues.
  net.sendFirst({ cmd = 'view' })
  app.resync = true
  app.debugMenu:connected()
end

net.onResponse = function(response, request)
  local view = response.view
  app.view = view
  app.response = response
  if not view then return end
  local events = response.events or {}
  app.events(events)
  -- The debug menu keeps the debug state of each response that has it.
  local fromMenu = app.debugMenu:response(response, request)
  if not response.ok then
    local err = response.error or {}
    local cmd = request and request.cmd or '?'
    app.refusals[#app.refusals + 1] = { cmd = cmd, code = err.code }
    -- A test script lists the refusals that it expects. Each other refusal stops the script.
    if runner and script.expected(runner, cmd, err.code) then
      print(('expected: the core does not accept %s (%s)'):format(cmd, tostring(err.code)))
    else
      io.stderr:write(('The core refused %s: %s (%s)\n'):format(cmd, tostring(err.code), tostring(err.message)))
      if runner then error(('The core refused %s (%s), and the script did not expect it'):format(cmd, tostring(err.code))) end
    end
    -- A refusal of a request of the debug menu changes nothing, and the screen did not ask for it.
    if fromMenu then return end
    if app.screen and app.name == view.screen then app.screen:refused(view, request) else enter(view, {}) end
    return
  end
  -- These two commands change nothing. A screen must not act a second time on the same view.
  if request and (request.cmd == 'hello' or request.cmd == 'debug_state') then return end
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

-- The rectangles of the notices: a column at the top of the stage, above the layout.
local function noticeRects()
  local list, y = {}, 0.75
  for i, notice in ipairs(app.notices) do
    local r = ui.noticeLayout(notice, layout.stage, y)
    list[i] = r
    y = y + r.h + 0.5
  end
  return list
end

--[[
  The chip of the debug menu: a key in the corner of the window, next to the name of the effects mode. Its position and
  its size are in pixels of the window. Its place is after the longest name of an effects mode, thus it does not move.
]]
local function chip()
  local font = theme.font('body', 13)
  local widest = 0
  for _, mode in ipairs(shaders.MODES) do widest = math.max(widest, font:getWidth(mode.label .. '  (F1)')) end
  return 6 + widest + 14 + 6, lg.getHeight() - 26, font:getWidth('Debug (F2)') + 14, 20
end

local function onChip(x, y)
  if not app.options.debug or not x then return false end
  local cx, cy, cw, ch = chip()
  return x >= cx and x < cx + cw and y >= cy and y < cy + ch
end

-- The debug menu opens only when the core has debug commands and the game has a screen.
local function toggleMenu()
  if app.options.debug and net.state == 'connected' and app.screen then app.debugMenu:toggle() end
end

-- The control at a point of the stage. A notice is above all. The debug menu is above the dialog, and the dialog is
-- above the screen.
function app.hit(x, y)
  if not x then return nil end
  for i, r in ipairs(noticeRects()) do
    if layout.contains(r, x, y) then return 'notice' .. i end
  end
  if net.state ~= 'connected' or not app.screen then return nil end
  if app.debugMenu.isOpen then return app.debugMenu:hit(x, y) end
  if app.dialog then
    local d = ui.dialogLayout(layout.stage, app.dialog.question)
    if layout.contains(d.ok, x, y) then return 'ok' end
    if layout.contains(d.cancel, x, y) then return 'cancel' end
    return nil
  end
  return app.screen:hit(x, y)
end

-- The rectangle of a named control, for the test script.
function app.control(name, arg)
  if name == 'notice' then return assert(noticeRects()[arg or 1], 'No notice ' .. tostring(arg)) end
  if name == 'debugChip' then
    local x, y, w, h = chip()
    local left, top = toStage(x, y)
    local right, bottom = toStage(x + w, y + h)
    return { x = left, y = top, w = right - left, h = bottom - top }
  end
  if app.debugMenu.isOpen then
    return assert(app.debugMenu:control(name, arg), ('The debug menu has no control %s %s'):format(tostring(name), tostring(arg)))
  end
  if app.dialog then
    local d = ui.dialogLayout(layout.stage, app.dialog.question)
    return assert(d[name], 'The dialog has no control ' .. tostring(name))
  end
  local r = app.screen and app.screen:control(name, arg)
  return assert(r, ('The screen %s has no control %s %s'):format(tostring(app.name), tostring(name), tostring(arg)))
end

local function activate(name)
  if name:find('^notice') then
    table.remove(app.notices, tonumber(name:sub(7)))
    return
  end
  if name == debugmenu.CHIP then return toggleMenu() end
  if ofMenu(name) then return app.debugMenu:activate(name) end
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
  -- The core accepts the debug commands unless the player gives --no-debug.
  options.debug = true
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
    elseif name == '--no-debug' then options.debug = false
    elseif name == '--no-save' then options.noSave = true
    elseif name == '--keep-alive' then options.keepAlive = true
    elseif name == '--novsync' then options.novsync = true
    elseif name == '--no-auth' then options.noAuth = true
    elseif name == '--embed' then options.embed = true
    elseif name == '--background' then shaders.setBackground(value())
    elseif name == '--post' then shaders.setPost(value())
    end
    i = i + 1
  end
end

function love.load(args)
  parse(args)
  local options = app.options
  -- A page of a browser cannot start a program or open a socket, thus the core is always in the game there.
  if love.system.getOS() == 'Web' then options.embed = true end
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
  app.saveDir = options.saveDir or love.filesystem.getSaveDirectory()
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
  -- A long frame (a screenshot, a move of the window) does not skip the motion. The screen below the debug menu
  -- continues, with no pointer.
  if app.screen then app.screen:update(math.min(dt, 0.05), app.debugMenu.isOpen and NO_POINTER or app.pointer) end
  app.debugMenu:update(dt)
  local t = love.timer.getTime()
  for i = #app.notices, 1, -1 do
    if t - app.notices[i].at > NOTICE_TIME then table.remove(app.notices, i) end
  end
  if runner then script.update(runner, app) end
end

local function moved(x, y)
  local p = app.pointer
  p.x, p.y = x, y
  p.hover = app.hit(x, y)
end

-- The pointer is at a point of the window. A point on the chip of the debug menu is not a point of the stage.
local function movedTo(x, y)
  if onChip(x, y) then
    local p = app.pointer
    p.x, p.y, p.hover = nil, nil, debugmenu.CHIP
    return
  end
  moved(toStage(x, y))
end

function love.mousemoved(x, y)
  movedTo(x, y)
end

function love.mousepressed(x, y, button)
  if button ~= 1 then return end
  movedTo(x, y)
  local p = app.pointer
  -- A notice and the debug menu take a click at each time. The screen takes a click only after its fade.
  local ready = app.inputReady() or ofMenu(p.hover) or (p.hover and p.hover:find('^notice'))
  p.pressed, p.pressedOn = ready and p.hover or nil, app.screen
end

-- A click is a press and a release on the same control of the same screen. The debug menu stays when the screen changes.
function love.mousereleased(x, y, button)
  if button ~= 1 then return end
  local p = app.pointer
  local pressed, on = p.pressed, p.pressedOn
  p.pressed, p.pressedOn = nil, nil
  movedTo(x, y)
  if pressed and pressed == p.hover and (on == app.screen or ofMenu(pressed)) then activate(pressed) end
  -- The click can change the controls, thus the control below the pointer can be a different one.
  movedTo(x, y)
end

function love.mousefocus(focus)
  if not focus then moved(nil, nil) end
end

function love.keypressed(key)
  if key == 'f1' then
    shaders.cycle()
    app.debugMenu.cache = nil
    return
  end
  if key == 'f2' then return toggleMenu() end
  -- While the debug menu is open, the dialog and the screen get no key.
  if app.debugMenu.isOpen then
    app.debugMenu:key(key)
    return
  end
  if not app.inputReady() then return end
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
    local open = app.debugMenu.isOpen
    local pointer = app.pointer
    if open then pointer = NO_POINTER elseif app.dialog then pointer = { x = pointer.x, y = pointer.y } end
    app.screen:draw(pointer)
    -- A new screen comes into view from the background.
    local t = (love.timer.getTime() - app.enteredAt) / TRANSITION
    if t < 1 then gfx.rect(-1, -1, layout.stage.w + 2, layout.stage.h + 2, 0, C.bg, 1 - gfx.ease(t)) end
    if app.dialog then ui.dialog(layout.stage, app.dialog.question, open and NO_POINTER or app.pointer) end
    if open then app.debugMenu:draw(app.pointer) end
  end
  connection()
  local t = love.timer.getTime()
  for i, r in ipairs(noticeRects()) do
    local notice = app.notices[i]
    ui.notice(notice, r, app.pointer.hover == 'notice' .. i, t - notice.at, NOTICE_TIME)
  end
  lg.pop()
  shaders.endScene()
  -- The name of the effects mode, in the corner of the window.
  local font = theme.font('body', 13)
  lg.setColor(0, 0, 0, 0.55)
  local label = shaders.current().label .. '  (F1)'
  lg.rectangle('fill', 6, lg.getHeight() - 26, font:getWidth(label) + 14, 20, 4, 4)
  lg.setColor(0.91, 0.90, 0.88, 0.9)
  lg.print(label, font, 13, lg.getHeight() - 24)
  -- The chip of the debug menu. Its text is amber while the menu is open.
  if app.options.debug then
    local x, y, w, h = chip()
    local ink = app.debugMenu.isOpen and C.accent or C.text
    lg.setColor(0, 0, 0, app.pointer.hover == debugmenu.CHIP and 0.8 or 0.55)
    lg.rectangle('fill', x, y, w, h, 4, 4)
    lg.setColor(ink[1], ink[2], ink[3], 0.9)
    lg.print('Debug (F2)', font, x + 7, y + 2)
  end
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

-- The set areas of each screen, in stage units, for the dump.
local AREAS = {
  title = layout.title, upgrades = layout.upgrades, relics = layout.relics, camp = layout.camp, over = layout.over,
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
    debug = app.debugMenu:state(),
    notices = (function()
      local list = {}
      for i, n in ipairs(app.notices) do list[i] = { kind = n.kind, title = n.title, lines = n.lines } end
      return list
    end)(),
    hover = app.pointer.hover or json.null,
    refusals = app.refusals,
    net = { state = net.state, address = net.address or json.null, stats = net.stats, pid = net.pid() or json.null,
      authenticated = net.authenticated, problem = net.problem or json.null },
    areas = app.name and rects(AREAS[app.name], '') or json.null,
    origin = { x = originX, y = originY },
    measurements = app.measurements or {},
    effects = shaders.current().label,
    background = shaders.background,
    post = shaders.post,
    window = { width = lg.getWidth(), height = lg.getHeight(), unit = gfx.u },
    fps = { now = love.timer.getFPS(), average = app.frames.count / math.max(seconds, 0.001), frames = app.frames.count,
      seconds = seconds, worstFrameMs = app.frames.worstMs, worstWorkMs = app.frames.worstWorkMs or 0,
      averageWorkMs = (app.frames.workTotal or 0) / math.max(1, app.frames.count - 5), fontsMade = theme.fontsMade },
  }, '  ') .. '\n'
end

_G.chrogue = app
return app
