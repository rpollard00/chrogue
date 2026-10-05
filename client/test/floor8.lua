--[[
  The boss of floor 8, with 10 relics: the banner, the fans, and 12 moves by clicks. The boss has level 11, the strongest
  level of the AI, thus the core needs time for each enemy move. The dump has the longest frame while the core selects each
  enemy move (client.enemyMove.worstFrameMs) and the time of each enemy_move request.
  Run with: --seed 7 --debug --no-save --script test/floor8.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)

local RELICS = { 'forcedMarch', 'backpedal', 'earlyPromo', 'kingKnight', 'longLeap', 'sidestep', 'bounty', 'secondWind', 'conscription', 'interest' }
local VALUE = { q = 9, r = 5, b = 3, n = 3, p = 1, k = 0 }

-- The move of the test player: the capture of the most valuable piece, else the first pawn move, else the first move.
local function choose(view)
  local at = {}
  for _, p in ipairs(view.pieces) do at[p.square] = p end
  local best, score = nil, -1
  for _, m in ipairs(view.moves) do
    local victim = at[m.to]
    local s = m.capture and 10 + (victim and VALUE[victim.kind] or 1) or (at[m.from].kind == 'p' and 1 or 0)
    if s > score then best, score = m, s end
  end
  return best
end

local steps = {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_floor', floor = 8 } },
  { 'send', { cmd = 'debug_tune', floor = 8, level = 11 } },
}
for _, id in ipairs(RELICS) do steps[#steps + 1] = { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end
local rest = {
  { 'settle' }, { 'wait', 0.5 }, { 'screenshot', out .. 'boss-banner' .. size .. '.png' }, { 'wait', 1.6 },
  { 'hover', 'medal', 'enemy', 2 }, { 'wait', 0.4 }, { 'screenshot', out .. 'boss-trait-card' .. size .. '.png' },
  { 'hover', 'medal', 'player', 10 }, { 'wait', 0.4 }, { 'screenshot', out .. 'boss-relic-card' .. size .. '.png' },
  { 'hover', 'none' },
  { 'expect', function(v) return v.floor.number == 8 and v.floor.boss and #v.relics == 10 and #v.traits == 2 end, 'floor 8 with 10 relics and two traits' },
}
for _, s in ipairs(rest) do steps[#steps + 1] = s end
for _ = 1, 12 do
  steps[#steps + 1] = { 'play', choose }
  steps[#steps + 1] = { 'settle' }
end
steps[#steps + 1] = { 'wait', 0.3 }
steps[#steps + 1] = { 'screenshot', out .. 'boss-after' .. size .. '.png' }
steps[#steps + 1] = { 'dump', out .. 'floor8' .. size .. '.json' }
steps[#steps + 1] = { 'expect', function(v, c)
  local e = c.enemyMove
  print(('enemy moves: %d, longest enemy_move round trip %.1f ms, longest frame while the core thinks %.1f ms'):format(e.count, e.worstMs, e.worstFrameMs))
  return e.count >= 6 and e.worstFrameMs < 50, ('%d moves'):format(e.count)
end, 'no frame longer than 50 ms while the core selects the enemy move' }
steps[#steps + 1] = { 'quit' }
return steps
