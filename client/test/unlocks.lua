--[[
  The relics screen, at 1440 by 900.
  - A new save: 8 relics of 27. A slot of a relic that crowns buy, a slot of a relic of a feat, and a slot of a relic
    that the player has.
  - A purchase with the key and a purchase with Enter. The purse counts down, and the slot shows the relic.
  - Each relic unlocked, with the relic of the longest text in the panel: the largest content of the panel.

  Run with: --size 1440x900 --seed 7 --no-save --script test/unlocks.lua
  The debug commands only set the crowns and unlock the relics. Each purchase is a click or a key.
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
  { 'screen', 'title' },
  { 'send', { cmd = 'open_relics' } }, { 'screen', 'relics' },
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

  -- Each relic unlocked. The ids of the locked relics come from `hello`: the view of the screen does not have them.
  { 'send', { cmd = 'hello' } }, { 'response' },
  expect('the test unlocks each relic', function(v, c, app)
    for _, relic in ipairs(app.response.data.content.relics) do
      if relic.unlock ~= 'start' then app.send({ cmd = 'debug_set_unlock', relic = relic.id, unlocked = true }) end
    end
    return true
  end),
  { 'settle' },
  expect('each relic is unlocked', function(v, c)
    local longest = 0
    for i, s in ipairs(v.slots) do
      if s ~= json.null and #s.text > longest then longest, longSlot[3] = #s.text, i end
    end
    return v.unlocked == 27 and c.unlocked == 27, ('unlocked %d'):format(v.unlocked)
  end),
  longSlot, { 'hover', 'none' }, { 'wait', 0.1 }, shot('all'), dump('all'),
  { 'press', 'back' }, { 'screen', 'title' },
  { 'quit' },
}
