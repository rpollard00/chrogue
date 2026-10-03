--[[
  The states with the largest content: the camp before the last boss (16 enemy pieces, two traits), a full army, each
  relic, blocked shop cards, the relic cards in the camp, and a won run. The debug commands prepare the positions;
  each move is a click. Run with: --seed 7 --debug --no-save --script test/showcase.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end
-- With CHROGUE_EFFECTS=off, the script starts with the effects off (two presses of F1), for the test of the layout.
local start = os.getenv('CHROGUE_EFFECTS') == 'off' and { { 'key', 'f1' }, { 'key', 'f1' } } or {}

local RELICS = { 'forcedMarch', 'backpedal', 'earlyPromo', 'kingKnight', 'longLeap', 'sidestep', 'bounty', 'secondWind', 'conscription', 'interest' }
-- Conscription puts a pawn on a free home square (a2), thus the mate is on the h-file: Rh1-h8.
local MATE = { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 48 }, { kind = 'p', square = 49 } } }
local ROOK = { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'r', home = 7 } } }
local FULL = { cmd = 'debug_set_army', units = {} }
for s = 0, 15 do
  local kinds = { 'r', 'n', 'b', 'q', 'k', 'b', 'n', 'r' }
  FULL.units[#FULL.units + 1] = { kind = s < 8 and kinds[s + 1] or 'p', home = s }
end

local steps = {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_floor', floor = 7 } },
}
for _, id in ipairs(RELICS) do steps[#steps + 1] = { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end
for _, s in ipairs({
  { 'send', ROOK }, { 'send', MATE }, { 'settle' }, { 'wait', 2 },
  { 'click', 'h1' }, { 'click', 'h8' }, { 'settle' }, { 'wait', 2.2 }, shot('battle-victory-relics'),
  expect('floor 7 is won', function(v) return v.result.outcome == 'victory' and v.result.next == 'camp' end),
  { 'press', 'continue' }, { 'screen', 'camp' },
  { 'send', FULL },
  { 'send', { cmd = 'debug_set_shop', offers = { { kind = 'piece', type = 'b' }, { kind = 'relic', id = 'bounty' }, { kind = 'gold', amount = 12 } } } },
  { 'settle' }, { 'wait', 0.5 }, shot('camp-boss'), dump('camp-boss'),
  expect('the camp before the last boss', function(v)
    return v.floor.number == 8 and #v.enemy.kinds == 16 and #v.enemy.traits == 2 and #v.army == 16 and #v.relics == 10
      and v.shop.offers[1].blocked == 'army_full' and v.shop.offers[2].blocked == 'owned' and v.reward.offers[1].blocked == 'army_full'
  end),
  { 'hover', 'medal', 'player', 10 }, { 'wait', 0.4 }, shot('camp-relic-card'),
  { 'hover', 'medal', 'enemy', 1 }, { 'wait', 0.4 }, shot('camp-trait-card'),
  { 'hover', 'control', 'card', { 'shop', 3 } }, { 'wait', 0.3 }, shot('camp-card-hover'),
  { 'hover', 'none' },
  { 'click', 'e1' },
  expect('the king is selected', function(v, c) return c.selected == 4 end),
  { 'key', 'escape' },
  expect('Escape clears the selection', function(v, c) return c.selected == -1 end),
  { 'press', 'skip' }, { 'settle' }, { 'press', 'start' }, { 'screen', 'battle' },
  { 'wait', 0.5 }, shot('battle-boss-banner'), { 'wait', 1.6 },
  { 'send', ROOK }, { 'send', MATE }, { 'settle' }, { 'wait', 2 },
  { 'click', 'h1' }, { 'click', 'h8' }, { 'settle' }, { 'wait', 1.5 },
  expect('floor 8 is won and the run is won', function(v) return v.result.outcome == 'victory' and v.result.next == 'won' end),
  { 'press', 'continue' }, { 'screen', 'over' }, shot('over-won'), dump('over-won'),
  expect('the run is won', function(v)
    return v.summary.won and v.summary.cleared == 8 and #v.rows == 2 and v.rows[2].row == 'win'
  end),
  { 'key', 'return' }, { 'screen', 'battle' },
  expect('Enter starts a new run', function(v) return v.floor.number == 1 end),
  { 'quit' },
}) do steps[#steps + 1] = s end
for i = #start, 1, -1 do table.insert(steps, 1, start[i]) end
return steps
