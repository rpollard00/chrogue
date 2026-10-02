-- The LÖVE port of the Chrogue battle screen. This file has the window, the frame, and the command line.
-- README.md tells how to build the rules and how to run the port.
local board = require('board')
local fan = require('fan')
local gfx = require('gfx')
local input = require('input')
local layout = require('layout')
local plaques = require('plaques')
local result = require('result')
local rules = require('rules')
local scenario = require('scenario')
local screen = require('screen')
local script = require('script')
local shaders = require('shaders')
local theme = require('theme')
local lg = love.graphics

local app = {}
local ui = input.new()
local makeRun, runner
local originX, originY = 0, 0
local frames, started = 0, 0
local options = { demo = 'boss' }

-- Starts the prepared battle from its first position. Continue and Give up do this: the prototype has no camp.
local function restart()
  app.screen = screen.new(makeRun, scenario.meta())
  app.myFan = fan.new(app.screen.run.relics, 'player', layout.me.fan)
  app.foeFan = fan.new(app.screen.run.enemy.traits, 'enemy', layout.foe.fan)
  ui.pressed = nil
  input.moved(ui, app.screen, ui.x, ui.y)
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

local function parse(args)
  local i = 1
  while i <= #args do
    local name, value = args[i], args[i + 1]
    if name == '--size' then
      local w, h = tostring(value):match('^(%d+)x(%d+)$')
      options.width, options.height = tonumber(w), tonumber(h)
      i = i + 1
    elseif name == '--demo' then
      options.demo = value
      i = i + 1
    elseif name == '--script' then
      options.script = value
      i = i + 1
    elseif name == '--seed' then
      options.seed = tonumber(value)
      i = i + 1
    elseif name == '--novsync' then
      options.novsync = true
    end
    i = i + 1
  end
end

function love.load(args)
  parse(args)
  if options.width or options.novsync then
    local w, h = options.width or lg.getWidth(), options.height or lg.getHeight()
    love.window.setMode(w, h, { resizable = true, minwidth = 640, minheight = 360, vsync = options.novsync and 0 or 1 })
  end
  -- The enemy AI uses math.random. A seed gives the same replies in each session, for a test script.
  math.randomseed(options.seed or os.time())
  shaders.load()
  fit()
  makeRun = scenario.prepare(options.demo)
  restart()
  if options.script then
    runner = script.load(options.script)
    -- With a script, an error stops the port. It does not wait for a person.
    love.errorhandler = function(message)
      io.stderr:write('Error: ' .. debug.traceback(tostring(message), 2) .. '\n')
      return function() return 1 end
    end
  end
  started = love.timer.getTime()
end

function love.resize() fit() end

function love.update(dt)
  local self = app.screen
  -- A long frame (a screenshot, a move of the window) does not skip the motion.
  screen.update(self, math.min(dt, 0.05))
  local x, y = ui.x, ui.y
  if self.confirm then x, y = nil, nil end
  fan.update(app.myFan, x, y, self.time, dt)
  fan.update(app.foeFan, x, y, self.time, dt)
  if runner then script.update(runner, app) end
  frames = frames + 1
end

function love.mousemoved(x, y)
  local sx, sy = toStage(x, y)
  input.moved(ui, app.screen, sx, sy)
end

function love.mousepressed(x, y, button)
  if button ~= 1 then return end
  local sx, sy = toStage(x, y)
  input.pressed(ui, app.screen, sx, sy)
end

function love.mousereleased(x, y, button)
  if button ~= 1 then return end
  local sx, sy = toStage(x, y)
  input.released(ui, app.screen, sx, sy, restart)
  -- The click can change the controls, thus the control below the pointer can be a different one.
  input.moved(ui, app.screen, sx, sy)
end

function love.mousefocus(focus)
  if not focus then input.moved(ui, app.screen, nil, nil) end
end

function love.keypressed(key)
  if key == 'f1' then shaders.cycle()
  elseif key == 'escape' and app.screen.confirm then app.screen.confirm = false end
end

local function drawFan(f, flashes)
  local self = app.screen
  shaders.foil(fan.bounds(f), self.time, ui.x, ui.y, 0.8, function() fan.draw(f, flashes, self.time) end)
end

local function drawCard(f)
  local r = fan.cardRect(f)
  if not r then return end
  local self = app.screen
  local alpha, scale = fan.cardEnter(f, self.time)
  local area = { x = r.x - 0.5, y = r.y - 0.5, w = r.w + 1, h = r.h + 1.2 }
  shaders.foil(area, self.time, ui.x, ui.y, 1, function()
    lg.push()
    lg.translate(r.x + r.w / 2, r.y + r.h / 2)
    lg.scale(scale)
    lg.translate(-(r.x + r.w / 2), -(r.y + r.h / 2))
    gfx.withAlpha(alpha, function() fan.drawCard(f, r) end)
    lg.pop()
  end)
end

function love.draw()
  local self = app.screen
  shaders.beginScene(love.timer.getTime() - started)
  lg.push()
  lg.translate(originX, originY)
  lg.scale(gfx.u)
  -- The layout: the enemy plaque, the board, and the player plaque.
  plaques.enemy(self, app.foeFan, function(f) drawFan(f, nil) end)
  plaques.player(self, app.myFan, function(f) drawFan(f, self.flashes) end, ui)
  board.draw(self, ui)
  result.draw(self, ui)
  -- The elements above the layout.
  if ui.stash and not self.confirm then plaques.stashList(self, ui.stash) end
  drawCard(app.foeFan)
  drawCard(app.myFan)
  result.drawConfirm(self, ui)
  lg.pop()
  shaders.endScene()
  -- The name of the effects mode, in the corner of the window.
  local font = theme.font('body', 13)
  lg.setColor(0, 0, 0, 0.55)
  local label = shaders.current().label .. '  (F1)'
  lg.rectangle('fill', 6, lg.getHeight() - 26, font:getWidth(label) + 14, 20, 4, 4)
  lg.setColor(0.91, 0.90, 0.88, 0.9)
  lg.print(label, font, 13, lg.getHeight() - 24)
end

-- The state of the battle as JSON, for the test script.
local function json(value, indent)
  indent = indent or ''
  local kind = type(value)
  if kind == 'table' then
    local inner, parts = indent .. '  ', {}
    if #value > 0 or next(value) == nil then
      for _, v in ipairs(value) do parts[#parts + 1] = json(v, inner) end
      return '[' .. table.concat(parts, ', ') .. ']'
    end
    local keys = {}
    for k in pairs(value) do keys[#keys + 1] = tostring(k) end
    table.sort(keys)
    for _, k in ipairs(keys) do parts[#parts + 1] = ('%s%q: %s'):format(inner, k, json(value[k], inner)) end
    return '{\n' .. table.concat(parts, ',\n') .. '\n' .. indent .. '}'
  elseif kind == 'string' then
    return ('%q'):format(value)
  elseif kind == 'number' then
    return value == math.floor(value) and ('%d'):format(value) or ('%.3f'):format(value)
  end
  return tostring(value == nil and 'null' or value)
end

function app.dump()
  local self = app.screen
  local state, battle = self.state, self.battle
  local rows = {}
  -- The board as the piece field of a FEN string. An upper case letter is a white piece.
  for r = 7, 0, -1 do
    local row, empty = '', 0
    for f = 0, 7 do
      local p = state.board[r * 8 + f + 1]
      if p then
        row = row .. (empty > 0 and empty or '') .. (p.color == 'w' and p.type:upper() or p.type)
        empty = 0
      else
        empty = empty + 1
      end
    end
    rows[#rows + 1] = row .. (empty > 0 and empty or '')
  end
  local targets = {}
  for _, m in ipairs(self.targets) do targets[#targets + 1] = m.to end
  table.sort(targets)
  local status, lit = screen.status(self)
  local seconds = love.timer.getTime() - started
  local res = battle.result
  return json({
    scenario = options.demo, floor = self.run.floor, enemy = self.spec.name, relics = self.run.relics, traits = self.run.enemy.traits,
    board = table.concat(rows, '/'), turn = state.turn, ep = state.ep, clock = state.clock, moves = self.moves,
    selected = self.selected, targets = targets, scouting = screen.scouting(self), canScout = self.scout,
    last = self.last and { from = self.last.from, to = self.last.to } or 'none',
    check = screen.checkSquare(self), busy = self.busy, promotion = self.promotion and #self.promotion or 0,
    status = status, lamp = lit, confirm = self.confirm,
    runGold = self.run.gold, captureGold = battle.gold, taken = { w = table.concat(battle.taken.w), b = table.concat(battle.taken.b) },
    lost = battle.lost, rescued = battle.rescued,
    result = res and { winner = res.winner or 'none', reason = res.reason, captures = res.reward.captures, clear = res.reward.clear,
      bonuses = res.reward.bonuses, total = rules.battle.totalGold(res.reward) } or 'none',
    hover = { control = ui.hover or 'none', playerMedal = app.myFan.hovered or 0, enemyMedal = app.foeFan.hovered or 0, stash = ui.stash or 'none' },
    effects = shaders.current().label,
    window = { width = lg.getWidth(), height = lg.getHeight(), unit = gfx.u },
    fps = { now = love.timer.getFPS(), average = frames / math.max(seconds, 0.001), frames = frames, seconds = seconds },
    search = { count = self.search.count, lastMs = self.search.last, worstMs = self.search.worst, averageMs = self.search.total / math.max(1, self.search.count) },
  }) .. '\n'
end
