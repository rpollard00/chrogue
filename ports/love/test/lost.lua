--[[
  The core stops in the middle of a battle. The game shows the panel of the lost connection, starts a new core with the
  same save folder, and connects again. The new core opens the title; the saved battle starts again from its start. Run with: --save-dir DIR --script test/lost.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' }, { 'wait', 2 },
  { 'click', 'e2' }, { 'click', 'e4' }, { 'settle' },
  { 'send', { cmd = 'quit' } }, { 'wait', 0.4 }, { 'screenshot', out .. 'lost' .. size .. '.png' },
  { 'expect', function(v, c, app) return require('net').state ~= 'connected' end, 'the game shows the lost connection' },
  { 'screen', 'title' }, { 'dump', out .. 'lost-after' .. size .. '.json' },
  { 'expect', function(v, c, app)
    local net = require('net')
    return net.stats.connects == 2 and v.can_continue and v.run.phase == 'battle', ('connects %d'):format(net.stats.connects)
  end, 'a new core has the saved run' },
  { 'press', 'continueRun' }, { 'screen', 'battle' },
  { 'expect', function(v) return v.floor.number == 1 and v.phase == 'player' end, 'the battle starts again from its start' },
  { 'quit' },
}
