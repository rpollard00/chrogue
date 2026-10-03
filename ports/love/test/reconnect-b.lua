--[[
  The second half of the reconnection test: the game connects to the core that continued (--connect HOST:PORT).
  The same battle continues: the client asks for the enemy move, and the player moves again. Then the core stops.
]]
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local function pieceAt(view, s)
  for _, p in ipairs(view.pieces) do if p.square == s then return p end end
end
return {
  { 'screen', 'battle' }, { 'dump', out .. 'reconnect-b.json' }, { 'screenshot', out .. 'reconnect-b.png' },
  -- The first game moved the knight g1-f3 and quit before the enemy move. This game asked for the enemy move.
  { 'expect', function(v, c, app)
    local moved, knight = v.last and pieceAt(v, v.last.to), pieceAt(v, 21)
    local event
    for _, e in ipairs(app.response.events or {}) do if e.type == 'move' then event = e end end
    return v.phase == 'player' and knight and knight.kind == 'n' and knight.color == 'w' and moved and moved.color == 'b'
      and event and event.color == 'b' and c.enemyMove.count == 1,
      ('last %s, enemy moves %d'):format(v.last and (v.last.from .. '-' .. v.last.to) or 'none', c.enemyMove.count)
  end, 'the battle continues, and the enemy moved after the reconnection' },
  { 'click', 'd2' }, { 'click', 'd3' }, { 'settle' }, { 'dump', out .. 'reconnect-c.json' },
  { 'expect', function(v) return v.phase == 'player' end, 'the player moved again, and the enemy replied' },
  { 'send', { cmd = 'quit' } }, { 'wait', 0.3 },
  { 'quit' },
}
