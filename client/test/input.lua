--[[
  Input across screen changes, and the set slots of the shop.
  - A double click on Continue after a won battle does not buy the shop card below the pointer.
  - A double click on Buy buys one item. The other cards keep their slots, and the bought card leaves an empty slot.
  - Two presses of Enter after a draw do not skip the camp.
  - A press on the battle board, then a new battle screen, then the release: the new screen does not take the click.
  Run with: --seed 7 --debug --no-save --script test/input.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end
local start = os.getenv('CHROGUE_EFFECTS') == 'off' and { { 'key', 'f1' }, { 'key', 'f1' } } or {}

local function count(list, cmd, from)
  local n = 0
  for i = from or 1, #list do if list[i] == cmd then n = n + 1 end end
  return n
end
local function same(a, b) return a.x == b.x and a.y == b.y and a.w == b.w and a.h == b.h end
local function shopRects(app)
  local list = {}
  for i = 1, #app.view.shop.offers do list[i] = app.control('card', { 'shop', i }) end
  return list
end

local goldAfterWin, sentAt, before, rewardBefore, price
local steps = {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_gold', gold = 200 } },
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'r', home = 7 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 48 }, { kind = 'p', square = 49 } } } },
  { 'settle' }, { 'wait', 2 }, { 'click', 'h1' }, { 'click', 'h8' }, { 'settle' }, { 'wait', 2.2 },
  expect('the battle is won', function(v, c, app)
    goldAfterWin, sentAt = v.gold + v.result.total, #app.sent
    return v.result.outcome == 'victory' and v.result.next == 'camp'
  end),

  -- The second click of a double click comes 0.1 seconds after the first, during the fade of the camp.
  { 'press', 'continue' }, { 'wait', 0.1 }, { 'again' }, { 'wait', 0.4 }, { 'settle' },
  expect('a double click on Continue opens the camp and buys nothing', function(v, c, app)
    return app.name == 'camp' and v.gold == goldAfterWin and #v.shop.offers == 4 and count(app.sent, 'buy', sentAt) == 0,
      ('screen %s, gold %s (expected %d), %d buy'):format(app.name, tostring(v.gold), goldAfterWin, count(app.sent, 'buy', sentAt))
  end),

  -- The shop: four items, and a double click on the Buy key of the first.
  { 'send', { cmd = 'debug_set_shop', offers = { { kind = 'piece', type = 'n' }, { kind = 'piece', type = 'b' }, { kind = 'relic', id = 'bounty' }, { kind = 'relic', id = 'interest' } } } },
  { 'settle' }, { 'wait', 0.3 }, shot('camp-before-buy'), dump('camp-before-buy'),
  expect('the shop has four cards in four slots', function(v, c, app)
    before, price, sentAt = shopRects(app), v.shop.offers[1].price, #app.sent
    return #before == 4 and v.shop.offers[1].affordable
  end),
  { 'press', 'buy', 1 }, { 'wait', 0.1 }, { 'again' }, { 'wait', 0.3 }, { 'settle' }, { 'hover', 'none' }, { 'wait', 0.2 },
  shot('camp-after-buy'), dump('camp-after-buy'),
  expect('a double click on Buy buys one item', function(v, c, app)
    return #v.shop.offers == 3 and v.gold == goldAfterWin - price and count(app.sent, 'buy', sentAt) == 1,
      ('%d items, gold %d, %d buy'):format(#v.shop.offers, v.gold, count(app.sent, 'buy', sentAt))
  end),
  expect('each card keeps its slot, and the bought card leaves an empty slot', function(v, c, app)
    local after = shopRects(app)
    for i = 1, 3 do
      if not same(after[i], before[i + 1]) then return false, ('card %d moved'):format(i) end
    end
    local key = before[1]
    return app.hit(key.x + key.w / 2, key.y + 12.4) == nil and c.shopSlots[1] == 2, 'the first slot is not empty'
  end),
  -- The reward cards also keep their slots.
  expect('the reward has three cards', function(v, c, app)
    rewardBefore = {}
    for i = 1, 3 do rewardBefore[i] = app.control('card', { 'reward', i }) end
    return v.reward.open and #v.reward.offers == 3
  end),
  { 'press', 'take', 2 }, { 'settle' },
  expect('the reward cards keep their slots after a take', function(v, c, app)
    for i = 1, 3 do
      if not same(app.control('card', { 'reward', i }), rewardBefore[i]) then return false, ('reward card %d moved'):format(i) end
    end
    return v.reward.state == 'taken'
  end),

  -- A draw: the camp after it has no reward, thus Enter there starts the next battle.
  { 'press', 'start' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 11 } } } },
  { 'settle' }, { 'wait', 2 }, { 'click', 'e1' }, { 'click', 'd2' }, { 'settle' }, { 'wait', 1.5 },
  expect('the battle is a draw', function(v, c, app) sentAt = #app.sent return v.result.outcome == 'draw' end),
  { 'key', 'return' }, { 'wait', 0.1 }, { 'key', 'return' }, { 'wait', 0.4 }, { 'settle' },
  expect('two presses of Enter after a draw open the camp and stay there', function(v, c, app)
    return app.name == 'camp' and v.can_start and count(app.sent, 'start_battle', sentAt) == 0, app.name
  end),

  -- A press on the board, a new battle screen, and the release.
  { 'press', 'start' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'p', home = 12 } } } },
  { 'settle' }, { 'wait', 2 },
  { 'down', 'square', 12 }, { 'send', { cmd = 'debug_set_relic', relic = 'bounty', on = true } }, { 'settle' }, { 'up' },
  expect('a release on a new screen is not a click', function(v, c) return c.selected == -1, ('selected %d'):format(c.selected) end),
  { 'click', 'e2' },
  expect('the next click selects the pawn', function(v, c) return c.selected == 12 end),
  { 'quit' },
}
for i = #start, 1, -1 do table.insert(steps, 1, start[i]) end
return steps
