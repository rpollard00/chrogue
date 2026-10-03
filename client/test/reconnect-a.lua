--[[
  The first half of the reconnection test: a battle starts, the player moves, and the game quits while the enemy waits.
  Run with: --keep-alive --seed 7 --no-save --script test/reconnect-a.lua. The core continues; the game prints its address.
]]
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' }, { 'wait', 2 },
  { 'click', 'e2' }, { 'click', 'e4' }, { 'settle' },
  { 'click', 'g1' }, { 'click', 'f3' }, { 'wait', 0.05 },
  { 'expect', function(v) return v.phase == 'enemy' end, 'the game quits while the enemy waits' },
  { 'dump', out .. 'reconnect-a.json' }, { 'screenshot', out .. 'reconnect-a.png' },
  { 'quit' },
}
