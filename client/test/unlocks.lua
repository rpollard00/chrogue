--[[
  The relics screen and the menu keys that open it, at 1440 by 900.
  - The menu of the title with no saved run and with a saved run, and the menu at the end of a run: a row of three keys, or of two keys that fill it.
  - A new save: 8 relics of 27. A slot of a relic that crowns buy, a slot of a relic of a feat, and a slot of a relic
    that the player has.
  - A purchase with the key and a purchase with Enter. The purse counts down, and the slot shows the relic.
  - A checkmate with a move of a rook, with no lost piece, does two feats. Each of the two relics gives a notice, and
    the notices stay in the camp.
  - The keys "Unlock all relics" and "Lock all relics" of the debug menu.
  - Each relic unlocked, with the relic of the longest text in the panel: the largest content of the panel.

  Run with: --size 1440x900 --seed 7 --no-save --script test/unlocks.lua
  The debug commands only set the crowns, the pieces of the battle, and the unlocked relics. Each purchase and each
  move is a click or a key.
]]
local json = require('json')

local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local function shot(name) return { 'screenshot', out .. 'unlocks-' .. name .. '.png' } end
local function dump(name) return { 'dump', out .. 'unlocks-' .. name .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end
local function has(app, kind)
  for _, e in ipairs(app.response and app.response.events or {}) do if e.type == kind then return e end end
end
-- The value of a field of a slot, or nil for `null`.
local function field(slot, name)
  local v = slot[name]
  if v == json.null then return nil end
  return v
end
-- The number of the first slot that the function accepts.
local function first(view, accept)
  for i, s in ipairs(view.slots) do
    if s ~= json.null and accept(s) then return i end
  end
end

-- The steps that press a slot. An `expect` step before them sets the number of the slot from the view.
local crownSlot, featSlot, secondSlot, longSlot = { 'press', 'slot', 1 }, { 'press', 'slot', 1 }, { 'press', 'slot', 1 }, { 'press', 'slot', 1 }
local cost, secondCost

return {
  { 'screen', 'title' }, { 'wait', 0.3 }, shot('title-new'),
  expect('the title with no saved run has two keys in the row', function(v, c)
    return not v.can_continue and table.concat(c.keys, ', ') == 'New run, Upgrades, Relics', table.concat(c.keys, ', ')
  end),
  { 'press', 'relics' }, { 'screen', 'relics' },
  expect('a new save has the 8 starter relics, and slot 1 is selected', function(v, c)
    crownSlot[3] = first(v, function(s) return field(s, 'cost') end)
    featSlot[3] = first(v, function(s) return field(s, 'feat') end)
    cost = field(v.slots[crownSlot[3]], 'cost')
    return v.unlocked == 8 and v.total == 27 and #v.slots == 36 and c.selected == 1 and c.unlocked == 8 and not c.canBuy
      and crownSlot[3] == 9 and featSlot[3] == 22 and v.slots[28] == json.null
  end),
  { 'wait', 0.3 }, shot('fresh-unlocked'), dump('fresh-unlocked'),
  crownSlot, { 'hover', 'none' }, { 'wait', 0.1 }, shot('fresh-crowns'),
  expect('a relic that crowns buy is hidden, and the player cannot buy it with no crowns', function(v, c)
    local s = v.slots[c.selected]
    return c.selected == crownSlot[3] and not c.canBuy and not s.unlocked and not field(s, 'id') and not field(s, 'name') and not field(s, 'text')
  end),
  { 'press', 'buy' }, { 'key', 'return' }, { 'settle' },
  expect('the key and Enter send no purchase', function(v, c, app) return v.unlocked == 8 and #app.refusals == 0 end),
  featSlot, { 'hover', 'none' }, { 'wait', 0.1 }, shot('fresh-feat'),
  expect('a relic of a feat shows only its condition', function(v, c)
    local s = v.slots[c.selected]
    return c.selected == featSlot[3] and not c.canBuy and field(s, 'feat') and not field(s, 'id') and not field(s, 'cost')
  end),
  { 'hover', 'control', 'slot', 2 }, { 'wait', 0.1 }, shot('fresh-hover'), { 'hover', 'none' },

  -- A purchase with the key.
  { 'send', { cmd = 'debug_set_crowns', crowns = 10 } }, { 'settle' },
  expect('the purse shows the crowns of the debug command', function(v, c) return v.meta.crowns == 10 and c.crownsShown == '10' end),
  crownSlot, { 'hover', 'none' }, { 'wait', 0.1 }, shot('affordable'),
  expect('the player can buy the relic', function(v, c) return c.canBuy and v.slots[c.selected].affordable end),
  { 'press', 'buy' }, { 'wait', 0.25 }, shot('buying'), { 'settle' }, { 'hover', 'none' }, { 'wait', 0.1 }, shot('bought'), dump('bought'),
  expect('the relic is unlocked, and the selection stays on its slot', function(v, c, app)
    local e, s = has(app, 'relic_unlocked'), v.slots[c.selected]
    secondSlot[3] = first(v, function(slot) return field(slot, 'cost') end)
    secondCost = field(v.slots[secondSlot[3]], 'cost')
    return e and field(e, 'feat') == nil and e.crowns_before == 10 and e.crowns == 10 - cost and v.meta.crowns == 10 - cost
      and c.crownsShown == tostring(10 - cost) and c.selected == crownSlot[3] and s.unlocked and s.id == e.id and s.name == e.name
      and v.unlocked == 9 and c.unlocked == 9 and not c.canBuy
  end),
  -- A purchase with Enter.
  secondSlot, { 'key', 'return' }, { 'settle' },
  expect('Enter buys the selected relic', function(v, c)
    return v.unlocked == 10 and v.meta.crowns == 10 - cost - secondCost and v.slots[c.selected].unlocked and c.selected == secondSlot[3]
  end),

  { 'press', 'back' }, { 'screen', 'title' },

  -- The title with a saved run, and the end of a run.
  { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'to_title' } }, { 'screen', 'title' }, { 'wait', 0.3 }, shot('title-saved'),
  expect('the title with a saved run has three keys in the row', function(v, c)
    return v.can_continue and table.concat(c.keys, ', ') == 'Continue run (floor 1), Upgrades, Relics, New run', table.concat(c.keys, ', ')
  end),
  { 'press', 'continueRun' }, { 'screen', 'battle' },

  -- Two feats in one battle: Ra1-a8 is checkmate, and the enemy captured no piece.
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'r', home = 0 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 63 }, { kind = 'p', square = 54 }, { kind = 'p', square = 55 } } } },
  { 'settle' }, { 'wait', 2 },
  { 'click', 'a1' }, { 'click', 'a8' }, { 'settle' },
  expect('the move that ends the battle unlocks two relics, and each gives a notice', function(v, c, app)
    local ids = {}
    for _, e in ipairs(app.response.events) do if e.type == 'relic_unlocked' then ids[#ids + 1] = e.id end end
    local n = app.notices
    return v.result.reason == 'checkmate' and table.concat(ids, ' ') == 'enfilade blessing' and #n == 2
      and n[1].title == 'Relic unlocked' and n[1].lines[1] == 'Enfilade can now come in a reward and in the shop.'
      and n[1].detail == 'Give checkmate with a move of a rook.' and n[2].detail == 'Win a battle and lose no piece.', table.concat(ids, ' ')
  end),
  { 'wait', 1.5 }, shot('notice-battle'),
  { 'press', 'continue' }, { 'screen', 'camp' }, shot('notice-camp'),
  { 'press', 'notice', 1 }, { 'press', 'notice', 1 },
  expect('a click closes a notice', function(v, c, app) return #app.notices == 0 end),
  { 'press', 'skip' }, { 'settle' }, { 'press', 'start' }, { 'screen', 'battle' }, { 'wait', 2 },
  { 'press', 'giveUp' }, { 'press', 'ok' }, { 'screen', 'over' }, { 'wait', 0.5 }, shot('over'),
  { 'press', 'relics' }, { 'screen', 'relics' }, { 'wait', 0.1 }, shot('after-feats'),
  expect('the relics screen opens from the end of a run, with the first relic that the player can buy', function(v, c, app)
    local done = 0
    for _, s in ipairs(v.slots) do if s ~= json.null and s.unlocked and (s.id == 'enfilade' or s.id == 'blessing') then done = done + 1 end end
    return v.unlocked == 12 and done == 2 and c.selected == first(v, function(s) return s.affordable end) and c.canBuy and #app.notices == 0,
      tostring(c.selected)
  end),

  -- The debug menu locks and unlocks each relic that a new save does not have.
  { 'key', 'f2' }, { 'settle' }, { 'press', 'tab', 'upgrades' },
  { 'press', 'lockAll' }, { 'settle' },
  expect('Lock all relics leaves the starter relics', function(v, c, app)
    local d = app.debugMenu.debug
    return v.unlocked == 8 and c.unlocked == 8 and #d.meta.relics == 0 and #d.meta.feats == 0, ('unlocked %d'):format(v.unlocked)
  end),
  { 'press', 'unlockAll' }, { 'settle' }, { 'wait', 0.1 }, shot('debug-keys'),
  { 'key', 'f2' },
  expect('each relic is unlocked', function(v, c)
    local longest = 0
    for i, s in ipairs(v.slots) do
      if s ~= json.null and #s.text > longest then longest, longSlot[3] = #s.text, i end
    end
    return v.unlocked == 27 and c.unlocked == 27, ('unlocked %d'):format(v.unlocked)
  end),
  longSlot, { 'hover', 'none' }, { 'wait', 0.1 }, shot('all'), dump('all'),
  { 'press', 'back' }, { 'screen', 'title' },
  expect('Back goes to the title', function(v) return v.screen == 'title' and not v.can_continue end),
  { 'quit' },
}
