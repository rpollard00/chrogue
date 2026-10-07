--[[
  The debug menu: the chip and F2, one control of each tab, and a command that the core does not accept.
  - A fixed seed of new runs gives the same first enemy for two new runs.
  - "Owned" adds a relic to the run, and "Offered" bars a relic.
  - The core does not accept a third trait or an 11th relic. The status line shows the message, and the game continues.
  - The stepper of the relic slots works with no run, and a new run has its slots.
  - A budget stepper changes the debug state, and an upgrade stepper changes the level of the upgrade.
  - The Effects tab selects a background, a post pass, and an effects mode.
  Run with: --seed 7 --no-save --script test/debug.lua
]]
local json = require('json')

local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end

local function count(list, cmd)
  local n = 0
  for _, sent in ipairs(list) do if sent == cmd then n = n + 1 end end
  return n
end
local function has(list, id)
  for _, item in ipairs(list) do if item == id or (type(item) == 'table' and item.id == id) then return true end end
  return false
end
-- The enemy pieces of a battle view, as one text.
local function enemy(view)
  local list = {}
  for _, p in ipairs(view.pieces) do if p.color == 'b' then list[#list + 1] = p.kind .. p.square end end
  table.sort(list)
  return table.concat(list, ' ')
end

local function relic(id) return { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end

local first, budget

return {
  { 'screen', 'title' },
  { 'press', 'debugChip' }, { 'settle' },
  expect('a click on the chip opens the menu, and the title gets no click', function(v, c, app)
    local menu = app.debugMenu
    return menu.isOpen and v.screen == 'title' and count(app.sent, 'new_run') == 0 and menu.debug ~= nil and menu.debug.run == nil
      and #menu.content.relics >= 10
  end),
  { 'key', 'escape' },
  expect('Escape closes the menu', function(v, c, app) return not app.debugMenu.isOpen end),
  { 'key', 'f2' }, { 'settle' },
  expect('F2 opens the menu', function(v, c, app) return app.debugMenu.isOpen and app.debugMenu.tab == 'run' end),

  -- Run: the relic slots, with no run.
  { 'press', 'relicSlots', '+' }, { 'settle' },
  expect('the stepper of the relic slots works with no run', function(v, c, app)
    local d = app.debugMenu.debug
    return d.run == nil and d.relic_slots == 5
  end),

  -- Run: a seed for new runs, and two new runs.
  { 'press', 'seedField' }, { 'key', '1' }, { 'key', '2' }, { 'key', '9' }, { 'key', 'backspace' }, { 'key', 'kp3' }, { 'key', '4' },
  { 'key', 'return' }, { 'settle' },
  expect('the seed field sets the seed of new runs', function(v, c, app) return app.debugMenu.debug.seed == 1234 end),
  { 'press', 'newRun' }, { 'screen', 'battle' },
  expect('New run starts a run with the seed', function(v, c, app)
    first = enemy(v)
    return app.debugMenu.isOpen and app.debugMenu.debug.run.seed == 1234 and v.floor.number == 1 and v.relic_slots == 5, first
  end),
  shot('debug-run'), dump('debug-run'),
  { 'press', 'newRun' }, { 'screen', 'battle' },
  expect('a second new run with the seed has the same first enemy', function(v, c, app)
    return count(app.sent, 'new_run') == 2 and count(app.sent, 'to_title') == 1 and app.debugMenu.debug.run.seed == 1234
      and enemy(v) == first, ('%s, then %s'):format(tostring(first), enemy(v))
  end),

  -- Relics: Owned, Offered, and a trait that the core does not accept.
  { 'press', 'tab', 'relics' }, { 'press', 'owned', 'bounty' }, { 'settle' },
  expect('Owned adds the relic to the run', function(v, c, app)
    return v.screen == 'battle' and has(v.relics, 'bounty') and has(app.debugMenu.debug.run.relics, 'bounty')
  end),
  { 'press', 'offered', 'interest' }, { 'settle' },
  expect('Offered bars the relic', function(v, c, app) return has(app.debugMenu.debug.barred, 'interest') end),
  { 'press', 'trait', 'forcedMarch' }, { 'settle' }, { 'press', 'trait', 'backpedal' }, { 'settle' },
  { 'refusal', 'debug_set_trait', 'blocked', 1 },
  { 'press', 'trait', 'earlyPromo' }, { 'settle' },
  expect('the status line has the message of the core, and the game continues', function(v, c, app)
    local state = json.decode(app.dump()).debug
    return #v.traits == 2 and state.open and state.tab == 'relics' and state.status == app.debugMenu.status and #state.status > 0,
      tostring(state.status)
  end),
  shot('debug-relics'), dump('debug-relics'),
  -- The run gets its 10 relics, and the menu asks for one more.
  relic('forcedMarch'), relic('backpedal'), relic('earlyPromo'), relic('kingKnight'), relic('longLeap'), relic('sidestep'),
  relic('secondWind'), relic('conscription'), relic('interest'), { 'settle' },
  { 'refusal', 'debug_set_relic', 'blocked', 1 },
  { 'press', 'owned', 'vault' }, { 'settle' },
  expect('the core does not accept a relic for a full run, and the status line tells it', function(v, c, app)
    local menu = app.debugMenu
    return #v.relics == menu.debug.limits.relics and not has(v.relics, 'vault') and menu.status ~= nil and #menu.status > 0,
      ('%d relics, status %s'):format(#v.relics, tostring(menu.status))
  end),

  -- Enemy: the budget of floor 1, then the defaults.
  { 'press', 'tab', 'enemy' },
  expect('the next action clears the status line', function(v, c, app)
    budget = app.debugMenu.debug.floors[1].budget
    return app.debugMenu.status == nil and not app.debugMenu.debug.tuned
  end),
  { 'press', 'budget', { 1, '+' } }, { 'settle' },
  expect('the budget stepper changes the budget of the floor', function(v, c, app)
    local d = app.debugMenu.debug
    return d.floors[1].budget == budget + 1 and d.tuned
  end),
  shot('debug-enemy'),
  { 'press', 'defaults' }, { 'settle' },
  expect('Defaults sets the default budget', function(v, c, app)
    local d = app.debugMenu.debug
    return d.floors[1].budget == budget and not d.tuned
  end),

  -- Upgrades: a level of the first upgrade.
  { 'press', 'tab', 'upgrades' }, { 'press', 'upgrade', { 'pawn', '+' } }, { 'settle' },
  expect('the upgrade stepper changes the level', function(v, c, app) return app.debugMenu.debug.meta.upgrades.pawn == 1 end),
  shot('debug-upgrades'),

  -- Effects: a background, a post pass, and an effects mode. The tab sends no command.
  { 'press', 'tab', 'effects' }, { 'press', 'background', 'walnutTour' }, { 'press', 'post', 'projection' }, { 'wait', 0.2 },
  expect('the Effects tab selects the background and the post pass', function()
    local shaders = require('shaders')
    return shaders.background == 'walnutTour' and shaders.post == 'projection' and shaders.mode == 1
  end),
  { 'press', 'mode', 2 },
  expect('the Effects tab selects the mode', function() return require('shaders').mode == 2 end),
  { 'wait', 0.2 }, shot('debug-effects'),
  { 'press', 'background', 'swirl' }, { 'press', 'post', 'crt' }, { 'press', 'mode', 1 },

  { 'press', 'close' },
  expect('Close closes the menu', function(v, c, app) return not app.debugMenu.isOpen end),
  { 'play', function(view) return view.moves[1] end }, { 'settle' },
  expect('the battle takes a move after the menu closes', function(v) return v.last ~= nil and v.phase == 'player' end),
  { 'quit' },
}
