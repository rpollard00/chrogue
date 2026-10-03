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
  -- The shutdown stops only the core: the number of the core is the core, and process 1 is not.
  { 'expect', function()
    local net = require('net')
    return net.isCore(net.pid()) and not net.isCore(1) and not net.isCore(999999)
  end, 'the game knows its core by its process number' },
  { 'press', 'continueRun' }, { 'screen', 'battle' },
  { 'expect', function(v)
    local at = {}
    for _, p in ipairs(v.pieces) do at[p.square] = p end
    -- The pawn that moved e2-e4 before the loss is on e2 again, and the battle has no last move.
    return v.floor.number == 1 and v.phase == 'player' and v.last == nil and at[12] and at[12].kind == 'p' and at[12].color == 'w'
      and at[28] == nil, ('last %s'):format(v.last and (v.last.from .. '-' .. v.last.to) or 'none')
  end, 'the battle starts again from its start position' },
  { 'quit' },
}
