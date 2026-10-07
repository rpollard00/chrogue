--[[
  The debug menu: a development tool. It is above the layout, as a dialog is, and it is not a part of the interface of the
  game. It changes the relics, the AI levels, the enemy armies, the seed of a run, and the upgrades with the debug commands
  of the core (core/PROTOCOL.md, "Debug commands").
  The menu draws only the state of the core: the debug state of the responses (`data.debug`) and the content of `hello`.
  It has no game rules, and the limits of its numbers come from the core. Each control sends one command.
  The Effects tab is different: it changes the shaders of the client, and it sends no command.
  The module has the name `debugmenu`, because `debug` is a library of Lua.
]]
local gfx = require('gfx')
local icons = require('icons')
local json = require('json')
local layout = require('layout')
local shaders = require('shaders')
local theme = require('theme')
local ui = require('ui')
local C, px = theme.color, gfx.px

local menu = {}
menu.__index = menu

-- Each control of the menu has a name that starts with this text, thus main.lua gives its clicks to the menu.
menu.PREFIX = 'debug:'
-- The name of the chip in the corner of the window.
menu.CHIP = 'debug:chip'

local function rect(x, y, w, h) return { x = x, y = y, w = w, h = h } end

-- The rows of each table of the menu have this step.
local ROW = 2.6

--[[
  The set layout of the menu, in stage units. The panel is in the center of the stage. Each tab has the rectangles of its
  largest content: 28 relics, 8 floors, 5 kinds, the 16 upgrades of the medal board, 11 backgrounds, and 2 post passes.
]]
local L = {
  panel = rect(4, 2.5, 72, 40),
  heading = rect(5.25, 3.5, 8, 2.25),
  tabs = { rect(14, 3.5, 7.6, 2.25), rect(22.1, 3.5, 7.6, 2.25), rect(30.2, 3.5, 7.6, 2.25), rect(38.3, 3.5, 7.6, 2.25),
    rect(46.4, 3.5, 7.6, 2.25) },
  close = rect(69.55, 3.5, 5.2, 2.25),
  rule = rect(5.25, 6.6, 69.5, px(1)),
  status = rect(5.25, 39.85, 69.5, 1.4),
  run = {
    seedsHead = rect(5.25, 7.5, 20, 1),
    thisLabel = rect(5.25, 9, 11.5, 2.25),
    thisSeed = rect(17, 9, 9, 2.25),
    useSeed = rect(26.7, 9, 10.3, 2.25),
    newLabel = rect(5.25, 12, 11.5, 2.25),
    -- The field has space for the 9 digits of the largest seed.
    field = rect(17, 12, 9, 2.25),
    set = rect(26.7, 12, 4.2, 2.25),
    random = rect(31.4, 12, 5.6, 2.25),
    -- The relic slots: a setting of the run in progress and of each new run.
    relicsHead = rect(5.25, 15.4, 20, 1),
    slotsLabel = rect(5.25, 16.9, 11.5, 2.25),
    slots = rect(17, 16.9, 9, 2.25),
    slotsNote = rect(26.7, 16.9, 12, 2.25),
    newRun = rect(17, 20.5, 20, 2.6),
    runHead = rect(42, 7.5, 20, 1),
    floorLabel = rect(42, 9, 5, 2.25),
    floor = rect(47, 9, 9.5, 2.25),
    floorName = rect(57.2, 9, 17.5, 2.25),
    goldLabel = rect(42, 12, 5, 2.25),
    gold = rect(47, 12, 9.5, 2.25),
  },
  relics = {
    count = rect(5.25, 7.5, 12, 2.25),
    traits = rect(17.75, 7.5, 12, 2.25),
    offerAll = rect(60.05, 7.5, 7, 2.25),
    offerNone = rect(67.55, 7.5, 7.2, 2.25),
    -- Two columns of 14 rows. The rows are nearer and lower than the rows of the other tabs.
    perColumn = 14, step = 2.05, height = 1.8,
    toggle = { w = 6.8, step = 7.25 },
  },
  -- The rows of the relics and of the upgrades: the left side of each of the two columns, and the top of the first row.
  columns = { 5.25, 40.75 },
  rowsY = 10.5,
  enemy = {
    headY = 7.5, headH = 1, rowsY = 9,
    -- The floors: the number and the name, the AI level (the well has space for "6  Lieutenant"), the budget, and
    -- the traits.
    floor = { name = { x = 5.25, w = 8.4 }, level = { x = 13.85, w = 13.8 }, budget = { x = 28.25, w = 7.4 }, traits = { x = 36.25, w = 7 } },
    -- The kinds: the piece and the name, the most pieces, the weight, and the first floor.
    kind = { name = { x = 46.15, w = 5.8 }, cap = { x = 52.15, w = 7 }, weight = { x = 59.75, w = 7.4 }, minFloor = { x = 67.75, w = 7 } },
    defaults = rect(5.25, 31, 7, 2.25),
    note = rect(13.25, 31, 61.5, 2.25),
  },
  upgrades = {
    crownsLabel = rect(5.25, 7.5, 5, 2.25),
    crowns = rect(10.5, 7.5, 10.5, 2.25),
    -- Two columns of 8 rows: the 16 slots of the medal board. The stepper has space for "3 of 3".
    perColumn = 8,
    level = { w = 10 },
  },
  effects = {
    backgroundHead = rect(5.25, 7.5, 20, 1),
    -- One column of 11 rows: the key of a background (it has space for "Walnut and tour"), and its note.
    rows = 11, rowsY = 9,
    background = { x = 5.25, w = 13 },
    note = { x = 19, w = 30 },
    modeHead = rect(52, 7.5, 20, 1),
    mode = { x = 52, w = 13 },
    modeNote = rect(52, 17.2, 22.75, 2.25),
    postHead = rect(52, 21, 20, 1),
    -- Two rows: the key of a post pass (it has space for "Rear projection"), and its note below the key.
    posts = 2, postsY = 22.5, postStep = 4.4,
    post = { x = 52, w = 13 },
    postNote = { x = 52, w = 22.75 },
  },
}

-- The parts of row `i` of the relics or of the upgrades: the medal, the name (it has space for "Close Quarters"), and the
-- place of the controls. `step` is the distance between two rows (ROW with no value), and `height` is the height of a
-- row (2 with no value). Nil for a row that the columns have no space for.
local function row(i, perColumn, step, height)
  local x = L.columns[math.floor((i - 1) / perColumn) + 1]
  if not x then return nil end
  local y, h = L.rowsY + ((i - 1) % perColumn) * (step or ROW), height or 2
  return { medal = { x = x + 1.2, y = y + 0.3 + h / 2, size = h }, name = rect(x + 2.9, y + 0.3, 9.2, h), x = x + 12.3, y = y + 0.3 }
end

local TABS = { { 'run', 'Run' }, { 'relics', 'Relics' }, { 'enemy', 'Enemy' }, { 'upgrades', 'Upgrades' }, { 'effects', 'Effects' } }
-- The core has no limit for the gold and for the crowns. The steppers stop at the largest numbers that the purses of the
-- game have space for: 4 digits of gold and 3 digits of crowns (layout.lua).
local GOLD_MAX, CROWNS_MAX = 9999, 999
local NOTE = 'A change of the army makes a new enemy for the floor of the run.'

function menu.new(app)
  return setmetatable({
    app = app, isOpen = false, tab = 'run', time = 0,
    -- The last debug state of the core, and the content of hello. Nil until the core gives them.
    debug = nil, content = nil, asked = false, asking = false,
    -- The floors and the kinds of a state that is not tuned, to mark the values that differ.
    defaults = nil,
    -- The message of the last refusal of the core, until the next action.
    status = nil,
    -- The seed field: true while it has the focus, and its digits.
    focus = false, buffer = '',
    -- The requests of the menu that wait for their responses.
    own = setmetatable({}, { __mode = 'k' }),
    cache = nil,
  }, menu)
end

function menu:send(request)
  self.own[request] = true
  if request.cmd == 'debug_state' then self.asking = true end
  self.app.send(request)
end

-- Asks for the content (one time) and for the debug state.
function menu:ask()
  if not self.asked then
    self.asked = true
    self:send({ cmd = 'hello' })
  end
  if not self.asking then self:send({ cmd = 'debug_state' }) end
end

function menu:open()
  self.isOpen, self.status, self.focus, self.cache = true, nil, false, nil
  self:ask()
end

function menu:close()
  self.isOpen, self.focus, self.cache = false, false, nil
end

function menu:toggle()
  if self.isOpen then self:close() else self:open() end
end

-- A new connection can be to a new core, which has the default settings.
function menu:connected()
  self.debug, self.defaults, self.asked, self.asking, self.cache = nil, nil, false, false, nil
  self.own = setmetatable({}, { __mode = 'k' })
  if self.isOpen then self:ask() end
end

--[[
  Each response of the core comes here first. The menu keeps the debug state and the content. Returns true if the
  request came from the menu. The message of a refusal of such a request goes to the status line.
]]
function menu:response(response, request)
  local data = response.ok and response.data or nil
  -- In the data of hello, `debug` is true or false. In the data of a debug command, it is the debug state.
  local state = data and type(data.debug) == 'table' and data.debug or nil
  if state then
    self.debug = state
    if state.tuned == false then self.defaults = { floors = state.floors, kinds = state.kinds } end
  end
  if data and data.content then self.content = data.content end
  local own = request ~= nil and self.own[request] == true
  if own then self.own[request] = nil end
  if request and request.cmd == 'debug_state' then self.asking = false end
  if own and not response.ok then
    local err = response.error or {}
    self.status = tostring(err.message or err.code)
  end
  -- A command of the game gives no debug state. While the menu is open, it asks for the state after such a command.
  if self.isOpen and response.ok and request and not state and request.cmd ~= 'hello' then self:ask() end
  self.cache = nil
  return own
end

-- Input

local function idOf(name, key) return menu.PREFIX .. name .. (key ~= nil and ':' .. key or '') end

-- The key of the argument of a named control: 'bounty', '+', or { 3, '+' }.
local function keyOf(arg)
  if type(arg) == 'table' then return table.concat(arg, ':') end
  if arg ~= nil then return tostring(arg) end
end

local function shift() return love.keyboard.isDown('lshift', 'rshift') end

function menu:setSeed()
  if self.buffer == '' or self.app.waiting() then return end
  self.status, self.focus = nil, false
  self:send({ cmd = 'debug_set_seed', seed = tonumber(self.buffer) })
end

-- Starts a new run from each screen, with no question: the commands that go to the title, then new_run.
function menu:newRun()
  local name = self.app.name
  if name == 'upgrades' then self:send({ cmd = 'back' })
  elseif name ~= 'title' then self:send({ cmd = 'to_title' }) end
  self:send({ cmd = 'new_run' })
end

function menu:update(dt) self.time = self.time + dt end

function menu:hit(x, y)
  local items = self:items()
  for i = #items, 1, -1 do
    local item = items[i]
    if item.id and layout.contains(item.rect, x, y) then return item.id end
  end
  return nil
end

function menu:find(id)
  for _, item in ipairs(self:items()) do
    if item.id == id then return item end
  end
end

-- The rectangle of a named control of the current tab, for the test script.
function menu:control(name, arg)
  local item = self:find(idOf(name, keyOf(arg)))
  return item and item.rect
end

-- A control that sends a command does nothing while a request waits for the core, as the controls of a screen.
function menu:activate(id)
  local item = self:find(id)
  if not item or item.disabled or not item.act then return end
  if item.sends and self.app.waiting() then return end
  self.status = nil
  if not item.field then self.focus = false end
  self.cache = nil
  item.act()
end

function menu:key(key)
  if key == 'escape' then
    -- Escape leaves the seed field first. Then it closes the menu.
    if self.focus then self.focus = false else self:close() end
  elseif self.focus then
    local digit = key:match('^kp(%d)$') or key:match('^%d$')
    if digit then
      local most = #('%d'):format(self.debug.limits.seed)
      if #self.buffer < most then self.buffer = self.buffer .. digit end
    elseif key == 'backspace' then self.buffer = self.buffer:sub(1, -2)
    elseif key == 'return' or key == 'kpenter' then self:setSeed() end
  end
  self.cache = nil
  return true
end

-- The elements

local function add(items, item)
  items[#items + 1] = item
  return item
end

-- One line of text in a set rectangle. opts: face, size, color, align, tracking.
local function label(items, text, r, opts)
  opts = opts or {}
  add(items, { rect = r, draw = function()
    gfx.text(text, opts.face or 'body', opts.size or 0.9, r.x, r.y,
      { color = opts.color, align = opts.align, width = r.w, line = r.h, tracking = opts.tracking })
  end })
end

local function kicker(items, text, r)
  label(items, text:upper(), r, { face = 'bold', size = 0.66, color = C.dim, tracking = 0.12 })
end

-- A value in a well.
local function value(items, r, text, color)
  add(items, { rect = r, draw = function()
    gfx.well(r.x, r.y, r.w, r.h, px(6))
    gfx.text(text, 'semibold', 0.9, r.x, r.y, { color = color, align = 'center', width = r.w, line = r.h })
  end })
end

-- A toggle: a key that shows its state. A toggle that is on has an amber ring, an amber lamp, and an amber label.
local function toggleKey(r, text, on, state, lamp)
  local function draw()
    local down = gfx.key(r.x, r.y, r.w, r.h, 'key', state)
    if on then gfx.outline(r.x, r.y + down, r.w, r.h, px(6), px(1.5), C.accent) end
    local lampW = lamp and 0.9 or 0
    local x = r.x + (r.w - lampW - gfx.textWidth(text, 'semibold', 0.8)) / 2
    if lamp then
      local cx, cy = x + 0.25, r.y + down + r.h / 2
      if on then gfx.circle(cx, cy, 0.25, C.accent) else gfx.ring(cx, cy, 0.25 - px(0.5), px(1), C.dim) end
    end
    gfx.text(text, 'semibold', 0.8, x + lampW, r.y + down, { color = on and C.accent or C.dim, line = r.h })
  end
  if state.disabled then gfx.withAlpha(0.45, draw) else draw() end
end

--[[
  A key of the menu. opts: act (the function of a click), disabled, sends (false for a key that sends no command),
  kind and size (as ui.key), on (true or false for a toggle), lamp (false for a toggle with no lamp), field (true for a
  key that keeps the focus of the seed field).
]]
local function key(items, name, arg, r, text, opts)
  local id = idOf(name, arg)
  add(items, { id = id, rect = r, act = opts.act, disabled = opts.disabled, sends = opts.sends ~= false, field = opts.field,
    draw = function(pointer)
      local state = ui.state(pointer, id, opts.disabled)
      if opts.on ~= nil then toggleKey(r, text, opts.on, state, opts.lamp ~= false)
      else ui.key(r, text, opts.kind or 'key', state, { size = opts.size }) end
    end })
end

--[[
  The stepper: [-] value [+]. Each number of the menu uses it. A side at its limit is disabled.
  opts: value (nil for no value), text (the text of the value), min, max, step (a number, or a function that gives it at
  the click), send (a function that gets the new value), color (of the text), disabled.
]]
local function stepper(items, name, arg, r, opts)
  local h = r.h
  local well = rect(r.x + h + 0.3, r.y, r.w - 2 * (h + 0.3), h)
  local v = not opts.disabled and opts.value or nil
  add(items, { rect = well, draw = function()
    gfx.well(well.x, well.y, well.w, well.h, px(6))
    if v then
      gfx.text(opts.text or tostring(v), 'semibold', 0.9, well.x, well.y, { color = opts.color, align = 'center', width = well.w, line = well.h })
    end
  end })
  local function side(sign, mark, text, x)
    local off = v == nil or (sign < 0 and v <= opts.min) or (sign > 0 and v >= opts.max)
    key(items, name, arg ~= nil and arg .. ':' .. mark or mark, rect(x, r.y, h, h), text, { disabled = off, size = 1.1, act = function()
      local step = type(opts.step) == 'function' and opts.step() or opts.step or 1
      opts.send(math.max(opts.min, math.min(opts.max, v + sign * step)))
    end })
  end
  side(-1, '-', '−', r.x)
  side(1, '+', '+', r.x + r.w - h)
end

local function medal(items, art, m, flair)
  add(items, { rect = rect(m.x - m.size / 2, m.y - m.size / 2, m.size, m.size), draw = function() ui.medal(art, m.x, m.y, m.size, flair) end })
end

local function set(list)
  local s = {}
  for _, v in ipairs(list or {}) do s[v] = true end
  return s
end

-- The tabs. Each function adds the elements of its tab from the debug state `d` and the content of hello.
local BUILD = {}

function BUILD.run(self, items, d)
  local R, run = L.run, d.run
  kicker(items, 'Seeds', R.seedsHead)
  label(items, 'Seed of this run', R.thisLabel)
  value(items, R.thisSeed, run and ('%d'):format(run.seed) or 'No run', run and C.text or C.dim)
  key(items, 'useSeed', nil, R.useSeed, 'Use for new runs', { disabled = not run or run.seed == d.seed,
    act = function() self:send({ cmd = 'debug_set_seed', seed = run.seed }) end })
  label(items, 'Seed of new runs', R.newLabel)
  local id, f = idOf('seedField'), R.field
  add(items, { id = id, rect = f, field = true,
    act = function() if not self.focus then self.focus, self.buffer = true, '' end end,
    draw = function(pointer)
      gfx.well(f.x, f.y, f.w, f.h, px(6))
      if self.focus then
        gfx.outline(f.x, f.y, f.w, f.h, px(6), px(1.5), C.accent)
        local w = gfx.text(self.buffer, 'semibold', 0.9, f.x + 0.7, f.y, { line = f.h })
        -- The caret is on for half of each second.
        if self.time % 1 < 0.5 then gfx.rect(f.x + 0.7 + w + px(1), f.y + 0.5, px(1.5), f.h - 1, 0, C.accent) end
      else
        if pointer.hover == id then gfx.outline(f.x, f.y, f.w, f.h, px(6), px(1), C.line) end
        gfx.text(d.seed and ('%d'):format(d.seed) or 'Random', 'semibold', 0.9, f.x + 0.7, f.y, { color = d.seed and C.text or C.dim, line = f.h })
      end
    end })
  key(items, 'setSeed', nil, R.set, 'Set', { field = true, disabled = not self.focus or self.buffer == '', act = function() self:setSeed() end })
  key(items, 'randomSeed', nil, R.random, 'Random', { disabled = d.seed == nil,
    act = function() self:send({ cmd = 'debug_set_seed', seed = json.null }) end })
  kicker(items, 'Relics', R.relicsHead)
  label(items, 'Relic slots', R.slotsLabel)
  stepper(items, 'relicSlots', nil, R.slots, { value = run and run.relic_slots or d.relic_slots, min = 0, max = d.limits.relics,
    send = function(v) self:send({ cmd = 'debug_set_relic_slots', slots = v }) end })
  label(items, 'This run and new runs', R.slotsNote, { size = 0.85, color = C.dim })
  key(items, 'newRun', nil, R.newRun, 'New run', { kind = 'primary', size = 1.1, act = function() self:newRun() end })

  kicker(items, 'Run in progress', R.runHead)
  label(items, 'Floor', R.floorLabel)
  stepper(items, 'floor', nil, R.floor, { value = run and run.floor, min = 1, max = #d.floors, disabled = not run,
    send = function(v) self:send({ cmd = 'debug_set_floor', floor = v }) end })
  local floor = run and d.floors[run.floor]
  if floor then label(items, floor.name, R.floorName, { color = C.dim }) end
  label(items, 'Gold', R.goldLabel)
  stepper(items, 'gold', nil, R.gold, { value = run and run.gold, min = 0, max = GOLD_MAX, step = 10, disabled = not run,
    send = function(v) self:send({ cmd = 'debug_set_gold', gold = v }) end })
end

function BUILD.relics(self, items, d, content)
  local R, run = L.relics, d.run
  local relics = content and content.relics or {}
  local owned, traits, barred = set(run and run.relics), set(run and run.traits), set(d.barred)
  -- The count of the relics against the relic slots: of the run, or of new runs with no run. It is amber when each
  -- slot has a relic.
  local count, slots = run and #run.relics or 0, run and run.relic_slots or d.relic_slots
  label(items, ('Relics %d of %d'):format(count, slots), R.count, { face = 'semibold', color = count >= slots and C.accent or nil })
  if run then
    label(items, ('Traits %d of %d'):format(#run.traits, d.limits.traits), R.traits, { face = 'semibold' })
  else
    label(items, 'No run', R.traits, { face = 'semibold', color = C.dim })
  end
  -- Each relic whose state differs gets one command.
  local function offer(all)
    for _, relic in ipairs(relics) do
      if (barred[relic.id] == true) == all then self:send({ cmd = 'debug_bar_relic', relic = relic.id, barred = not all }) end
    end
  end
  key(items, 'offerAll', nil, R.offerAll, 'Offer all', { disabled = #d.barred == 0, act = function() offer(true) end })
  key(items, 'offerNone', nil, R.offerNone, 'Offer none', { disabled = #d.barred >= #relics, act = function() offer(false) end })
  for i, relic in ipairs(relics) do
    local slot = row(i, R.perColumn, R.step, R.height)
    if not slot then break end
    medal(items, { kind = 'icon', id = relic.id }, slot.medal, C.relic)
    -- The pointer on the name shows the text of the relic.
    local name = slot.name
    add(items, { id = idOf('name', relic.id), rect = name, tip = relic, draw = function()
      gfx.text(relic.name, 'semibold', 0.9, name.x, name.y, { line = name.h })
    end })
    local function at(n) return rect(slot.x + (n - 1) * R.toggle.step, slot.y, R.toggle.w, name.h) end
    key(items, 'owned', relic.id, at(1), 'Owned', { on = owned[relic.id] == true, disabled = not run,
      act = function() self:send({ cmd = 'debug_set_relic', relic = relic.id, on = not owned[relic.id] }) end })
    key(items, 'offered', relic.id, at(2), 'Offered', { on = not barred[relic.id],
      act = function() self:send({ cmd = 'debug_bar_relic', relic = relic.id, barred = not barred[relic.id] }) end })
    -- Only a relic that has a text for the enemy can be a trait. The slot of each other relic stays empty.
    if relic.trait then
      key(items, 'trait', relic.id, at(3), 'Trait', { on = traits[relic.id] == true, disabled = not run,
        act = function() self:send({ cmd = 'debug_set_trait', relic = relic.id, on = not traits[relic.id] }) end })
    end
  end
end

function BUILD.enemy(self, items, d, content)
  local E, limits, defaults = L.enemy, d.limits, self.defaults
  local F, K = E.floor, E.kind
  local function head(text, column) kicker(items, text, rect(column.x, E.headY, column.w, E.headH)) end
  local function cell(column, y) return rect(column.x, y, column.w, 2) end
  -- A value that differs from its default has the amber color.
  local function mark(base, now, field) return base and base[field] ~= now[field] and C.accent or nil end
  head('Floor', F.name); head('AI level', F.level); head('Budget', F.budget); head('Traits', F.traits)
  for i, floor in ipairs(d.floors) do
    local y = E.rowsY + (i - 1) * ROW
    local base = defaults and defaults.floors[i]
    local function tune(field)
      return function(v) self:send({ cmd = 'debug_tune', floor = floor.number, [field] = v }) end
    end
    local n = F.name
    add(items, { rect = cell(n, y), draw = function()
      gfx.text(tostring(floor.number), 'display', 1, n.x, y, { color = C.dim, line = 2 })
      gfx.text(floor.name, 'semibold', 0.9, n.x + 1.2, y, { line = 2 })
    end })
    stepper(items, 'level', floor.number, cell(F.level, y), { value = floor.level, text = floor.level .. '  ' .. floor.level_name,
      min = 1, max = limits.level, send = tune('level'), color = mark(base, floor, 'level') })
    stepper(items, 'budget', floor.number, cell(F.budget, y), { value = floor.budget, min = 0, max = limits.budget,
      step = function() return shift() and 5 or 1 end, send = tune('budget'), color = mark(base, floor, 'budget') })
    stepper(items, 'traits', floor.number, cell(F.traits, y), { value = floor.traits, min = 0, max = limits.traits,
      send = tune('traits'), color = mark(base, floor, 'traits') })
  end
  local names = {}
  for _, piece in ipairs(content and content.pieces or {}) do names[piece.kind] = piece.name end
  head('Kind', K.name); head('Most', K.cap); head('Weight', K.weight); head('First floor', K.minFloor)
  for i, kind in ipairs(d.kinds) do
    local y = E.rowsY + (i - 1) * ROW
    local base = defaults and defaults.kinds[i]
    local function tune(field)
      return function(v) self:send({ cmd = 'debug_tune', kind = kind.kind, [field] = v }) end
    end
    local n = K.name
    add(items, { rect = cell(n, y), draw = function()
      -- An enemy piece is black, thus it is on the brown lining.
      gfx.well(n.x, y, 2, 2, px(6), true)
      gfx.piece(kind.kind, 'b', n.x + 1, y + 1, 1.4)
      gfx.text(names[kind.kind] or kind.kind:upper(), 'semibold', 0.9, n.x + 2.5, y, { line = 2 })
    end })
    stepper(items, 'cap', kind.kind, cell(K.cap, y), { value = kind.cap, min = 0, max = kind.cap_max, send = tune('cap'),
      color = mark(base, kind, 'cap') })
    stepper(items, 'weight', kind.kind, cell(K.weight, y), { value = kind.weight, text = ('%.1f'):format(kind.weight), min = 0,
      max = limits.weight, step = 0.5, send = tune('weight'), color = mark(base, kind, 'weight') })
    stepper(items, 'minFloor', kind.kind, cell(K.minFloor, y), { value = kind.min_floor, min = 1, max = #d.floors,
      send = tune('min_floor'), color = mark(base, kind, 'min_floor') })
  end
  key(items, 'defaults', nil, E.defaults, 'Defaults', { disabled = not d.tuned,
    act = function() self:send({ cmd = 'debug_tune', reset = true }) end })
  label(items, NOTE, E.note, { size = 0.85, color = C.dim })
end

function BUILD.upgrades(self, items, d, content)
  local U = L.upgrades
  label(items, 'Crowns', U.crownsLabel)
  stepper(items, 'crowns', nil, U.crowns, { value = d.meta.crowns, min = 0, max = CROWNS_MAX, step = 10,
    send = function(v) self:send({ cmd = 'debug_set_crowns', crowns = v }) end })
  for i, upgrade in ipairs(content and content.upgrades or {}) do
    local slot = row(i, U.perColumn)
    if not slot then break end
    local level = d.meta.upgrades[upgrade.id] or 0
    medal(items, icons.UPGRADE_ART[upgrade.id] or { kind = 'icon', id = upgrade.id }, slot.medal, C.upgrade)
    label(items, upgrade.name, slot.name, { face = 'semibold' })
    stepper(items, 'upgrade', upgrade.id, rect(slot.x, slot.y, U.level.w, slot.name.h), { value = level,
      text = ('%d of %d'):format(level, #upgrade.costs), min = 0, max = #upgrade.costs,
      send = function(v) self:send({ cmd = 'debug_set_upgrade', upgrade = upgrade.id, level = v }) end })
  end
end

-- The shaders of the client. This tab needs no state of the core.
function BUILD.effects(self, items)
  local E = L.effects
  kicker(items, 'Background', E.backgroundHead)
  for i, background in ipairs(shaders.BACKGROUNDS) do
    if i > E.rows then break end
    local y = E.rowsY + (i - 1) * ROW
    key(items, 'background', background.id, rect(E.background.x, y, E.background.w, 2.25), background.label,
      { on = shaders.background == background.id, sends = false, act = function() shaders.setBackground(background.id) end })
    label(items, background.note, rect(E.note.x, y, E.note.w, 2.25), { size = 0.85, color = C.dim })
  end
  kicker(items, 'Effects (F1)', E.modeHead)
  for i, mode in ipairs(shaders.MODES) do
    key(items, 'mode', i, rect(E.mode.x, E.rowsY + (i - 1) * ROW, E.mode.w, 2.25), mode.name,
      { on = shaders.mode == i, sends = false, act = function() shaders.mode = i end })
  end
  label(items, 'With all off, the game shows no background.', E.modeNote, { size = 0.85, color = C.dim })
  kicker(items, 'Screen', E.postHead)
  for i, post in ipairs(shaders.POSTS) do
    if i > E.posts then break end
    local y = E.postsY + (i - 1) * E.postStep
    key(items, 'post', post.id, rect(E.post.x, y, E.post.w, 2.25), post.label,
      { on = shaders.post == post.id, sends = false, act = function() shaders.setPost(post.id) end })
    label(items, post.note, rect(E.postNote.x, y + 2.25, E.postNote.w, 1.6), { size = 0.85, color = C.dim })
  end
end

-- The elements of the menu: the tabs, the Close key, and the elements of the current tab. Each element has `rect` and
-- `draw`. A control also has `id`. The list changes only with the state, thus it is kept until the next change.
function menu:items()
  if self.cache then return self.cache end
  local items = {}
  for i, tab in ipairs(TABS) do
    key(items, 'tab', tab[1], L.tabs[i], tab[2], { on = self.tab == tab[1], lamp = false, sends = false,
      act = function() self.tab = tab[1] end })
  end
  key(items, 'close', nil, L.close, 'Close', { sends = false, act = function() self:close() end })
  if self.debug or self.tab == 'effects' then BUILD[self.tab](self, items, self.debug, self.content) end
  self.cache = items
  return items
end

function menu:draw(pointer)
  local stage, p = layout.stage, L.panel
  gfx.rect(0, 0, stage.w, stage.h, 0, C.black, 0.45)
  gfx.shadow(p.x, p.y, p.w, p.h, px(10), px(10), px(40), 0, 0.6)
  gfx.rect(p.x, p.y, p.w, p.h, px(10), C.panel)
  gfx.outline(p.x, p.y, p.w, p.h, px(10), px(1), C.line)
  local h = L.heading
  gfx.text('DEBUG', 'display', 1.5, h.x, h.y, { tracking = 0.04, line = h.h })
  gfx.rect(L.rule.x, L.rule.y, L.rule.w, L.rule.h, 0, C.line)
  local tip
  for _, item in ipairs(self:items()) do
    item.draw(pointer)
    if item.tip and pointer.hover == item.id then tip = item end
  end
  if self.status then
    local s = L.status
    gfx.text(self.status, 'body', 0.85, s.x, s.y, { color = C.danger, line = s.h })
  end
  -- The paper tip of a relic is above the menu.
  if tip then ui.tip(tip.rect, tip.tip.name, tip.tip.text, stage.w) end
end

-- The state of the menu, for the dump of the test script.
function menu:state()
  return { open = self.isOpen, tab = self.tab, status = self.status or json.null, focus = self.focus, seedText = self.buffer,
    state = self.debug or json.null }
end

return menu
