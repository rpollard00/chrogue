--[[
  The second half of the reconnection test: the game connects to the core that continued (--connect HOST:PORT).
  The same battle continues: the client asks for the enemy move, and the player moves again. Then the core stops.
]]
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
return {
  { 'screen', 'battle' }, { 'dump', out .. 'reconnect-b.json' }, { 'screenshot', out .. 'reconnect-b.png' },
  { 'expect', function(v) return v.phase == 'player' and v.last ~= nil end, 'the battle continues, and the enemy moved' },
  { 'click', 'd2' }, { 'click', 'd3' }, { 'settle' }, { 'dump', out .. 'reconnect-c.json' },
  { 'expect', function(v) return v.phase == 'player' end, 'the player moved again, and the enemy replied' },
  { 'send', { cmd = 'quit' } }, { 'wait', 0.3 },
  { 'quit' },
}
