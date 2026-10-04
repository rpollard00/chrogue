--[[
  The relic slots of a run, and the discard of a relic in the camp.
  - A run has 4 relic slots. With 4 relics, a relic card of the shop has "Relics full".
  - A click on a medal selects it. A second click, a click on a different place, and Escape clear the selection.
  - The card of the selected medal is above a shop card. A click on it does not buy the item below it.
  - Discard asks first. Cancel keeps the relic. OK removes it, and the player can buy the relic card.
  Run with: --seed 7 --no-save --script test/relics.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end
local function relic(id) return { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end

local function count(list, cmd)
  local n = 0
  for _, sent in ipairs(list) do if sent == cmd then n = n + 1 end end
  return n
end
local function has(list, id)
  for _, item in ipairs(list) do if item.id == id then return true end end
  return false
end
local function event(app, kind)
  for _, e in ipairs(app.response and app.response.events or {}) do if e.type == kind then return e end end
end

local SECOND = { 'player', 2 }

return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  expect('a new run has 4 relic slots and no relic', function(v) return v.relic_slots == 4 and #v.relics == 0 end),
  { 'send', { cmd = 'debug_set_gold', gold = 60 } },
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'r', home = 7 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 48 }, { kind = 'p', square = 49 } } } },
  { 'settle' }, { 'wait', 2 }, { 'click', 'h1' }, { 'click', 'h8' }, { 'settle' }, { 'wait', 2.2 },
  { 'press', 'continue' }, { 'screen', 'camp' },
  relic('forcedMarch'), relic('bounty'), relic('interest'), relic('secondWind'),
  { 'send', { cmd = 'debug_set_shop', offers = { { kind = 'relic', id = 'conscription' }, { kind = 'piece', type = 'n' } } } },
  { 'settle' }, { 'wait', 0.8 }, { 'hover', 'none' }, shot('relics-full'), dump('relics-full'),
  expect('each slot has a relic, and the relic card is blocked', function(v, c)
    return v.relic_slots == 4 and #v.relics == 4 and v.shop.offers[1].blocked == 'relics_full' and c.relic == false
  end),

  -- The selection of a medal.
  { 'press', 'medal', SECOND },
  expect('a click on a medal selects it', function(v, c) return c.relic == 'bounty' end),
  { 'press', 'medal', SECOND },
  expect('a second click clears the selection', function(v, c) return c.relic == false end),
  { 'press', 'medal', SECOND }, { 'key', 'escape' },
  expect('Escape clears the selection of the medal', function(v, c) return c.relic == false end),
  { 'press', 'medal', SECOND }, { 'click', 'e1' },
  expect('a click on a unit clears the selection of the medal and selects the unit', function(v, c)
    return c.relic == false and c.selected == 4
  end),
  { 'press', 'medal', SECOND },
  expect('a click on a medal clears the selection of the unit', function(v, c) return c.relic == 'bounty' and c.selected == -1 end),
  { 'hover', 'none' }, { 'wait', 0.3 }, shot('relics-selected'),
  -- The card of the medal is above the key of the second shop card.
  { 'press', 'buy', 2 }, { 'settle' },
  expect('a click on the card of the medal buys nothing and clears the selection', function(v, c, app)
    return c.relic == false and count(app.sent, 'buy') == 0 and #v.shop.offers == 2
  end),

  -- The discard.
  { 'press', 'discard' },
  expect('Discard does nothing with no selection', function(v, c, app) return app.dialog == nil end),
  { 'press', 'medal', SECOND }, { 'press', 'discard' },
  expect('Discard asks first', function(v, c, app)
    return app.dialog ~= nil and app.dialog.question == 'Discard Bounty?\nYou get no gold for it.'
  end),
  { 'wait', 0.2 }, shot('relics-dialog'),
  { 'press', 'cancel' },
  expect('Cancel keeps the relic and the selection', function(v, c, app)
    return app.dialog == nil and #v.relics == 4 and c.relic == 'bounty' and count(app.sent, 'discard_relic') == 0
  end),
  { 'press', 'discard' }, { 'press', 'ok' }, { 'settle' },
  expect('OK removes the relic, and the relic card is free', function(v, c, app)
    local e = event(app, 'camp_action')
    return #v.relics == 3 and not has(v.relics, 'bounty') and c.relic == false and v.shop.offers[1].blocked == nil
      and e and e.action == 'discard_relic' and e.discarded[1] == 'bounty'
  end),
  { 'hover', 'none' }, { 'wait', 0.3 }, shot('relics-discarded'), dump('relics-discarded'),
  { 'press', 'buy', 1 }, { 'settle' },
  expect('the player buys the relic for the free slot', function(v) return #v.relics == 4 and has(v.relics, 'conscription') end),
  { 'quit' },
}
