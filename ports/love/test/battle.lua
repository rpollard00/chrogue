--[[
  The edges of a battle:
  - A battle that starts with no legal move for the player: the lamp tells it and points to Give up (checkmate, then stalemate).
  - Give up during the pause before the enemy move: no enemy_move follows give_up.
  - The core refuses enemy_move: the client asks one more time. After a second refusal, the lamp tells it.
  - A promotion to a named piece in the picker.
  Run with: --seed 7 --debug --no-save --script test/battle.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function expect(label, check) return { 'expect', check, label } end

local function after(list, cmd, from)
  local n = 0
  for i = from or 1, #list do if list[i] == cmd then n = n + 1 end end
  return n
end
local function pieceAt(view, s)
  for _, p in ipairs(view.pieces) do if p.square == s then return p end end
end
local function refusal(app)
  return { ok = false, error = { code = 'internal', message = 'A refusal of the test script' }, view = app.view, events = {} }, { cmd = 'enemy_move' }
end

local mark
return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  -- Checkmate at the start: the white king on e1, the black queen on e2, and the black king on e3 protects the queen.
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 20 }, { kind = 'q', square = 12 } } } },
  { 'settle' }, { 'wait', 2 }, shot('battle-no-move'),
  expect('checkmate at the start: the lamp points to Give up', function(v, c)
    return v.phase == 'player' and #v.moves == 0 and v.check ~= nil and c.stuck and c.status == 'No legal move. Give up.' and not c.lamp, c.status
  end),
  -- Stalemate at the start: the white king on a1, the black queen on b3.
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 0 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 63 }, { kind = 'q', square = 17 } } } },
  { 'settle' }, { 'wait', 0.3 },
  expect('stalemate at the start: the same status, with no check', function(v, c)
    return #v.moves == 0 and v.check == nil and c.status == 'No legal move. Give up.' and not c.lamp, c.status
  end),
  { 'press', 'giveUp' }, { 'wait', 0.25 }, { 'press', 'ok' }, { 'screen', 'over' },

  -- Give up while the enemy waits. The dialog is open longer than the pause.
  { 'press', 'newRun' }, { 'screen', 'battle' }, { 'wait', 2 },
  expect('a mark in the log of commands', function(v, c, app) mark = #app.sent + 1 return true end),
  { 'click', 'e2' }, { 'click', 'e4' }, { 'press', 'giveUp' }, { 'wait', 0.7 },
  expect('the enemy waits while the dialog is open', function(v, c, app)
    return app.dialog ~= nil and v.phase == 'enemy' and after(app.sent, 'enemy_move', mark) == 0
  end),
  { 'press', 'ok' }, { 'screen', 'over' }, { 'wait', 0.5 },
  expect('no enemy_move after give_up', function(v, c, app)
    return app.name == 'over' and after(app.sent, 'give_up', mark) == 1 and after(app.sent, 'enemy_move', mark) == 0,
      table.concat(app.sent, ' ', mark)
  end),

  -- The core refuses enemy_move one time: the client asks again, and the enemy moves.
  { 'press', 'newRun' }, { 'screen', 'battle' }, { 'wait', 2 },
  { 'refusal', 'enemy_move', 'internal', 3 },
  { 'click', 'e2' }, { 'click', 'e4' }, { 'wait', 0.1 },
  expect('the enemy waits', function(v, c, app) mark = #app.sent + 1 return v.phase == 'enemy' and c.waitingForEnemy end),
  { 'respond', refusal },
  expect('after one refusal, the client asks again', function(v, c) return c.enemyRefusals == 1 and c.waitingForEnemy and not c.enemyFailed end),
  { 'settle' },
  expect('the enemy moved after the second request', function(v, c, app)
    return v.phase == 'player' and c.enemyRefusals == 0 and after(app.sent, 'enemy_move', mark) == 1
  end),
  -- Two refusals: the client stops and the lamp tells it.
  { 'click', 'd2' }, { 'click', 'd4' }, { 'wait', 0.1 },
  expect('the enemy waits again', function(v, c, app) mark = #app.sent + 1 return v.phase == 'enemy' end),
  { 'respond', refusal }, { 'respond', refusal }, { 'wait', 1 }, shot('battle-enemy-failed'),
  expect('after two refusals, the client does not ask again', function(v, c, app)
    return c.enemyFailed and c.status == 'The enemy move failed.' and not c.lamp and not c.waitingForEnemy
      and after(app.sent, 'enemy_move', mark) == 0, c.status
  end),
  { 'press', 'giveUp' }, { 'wait', 0.25 }, { 'press', 'ok' }, { 'screen', 'over' },

  -- A promotion to a knight: the play step selects the piece of the move in the picker.
  { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'p', home = 15 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 49 } } } },
  { 'send', { cmd = 'debug_set_relic', relic = 'forcedMarch', on = true } },
  { 'send', { cmd = 'debug_set_relic', relic = 'earlyPromo', on = true } },
  { 'settle' }, { 'wait', 2 },
  { 'click', 'h2' }, { 'click', 'h4' }, { 'settle' }, { 'click', 'h4' }, { 'click', 'h6' }, { 'settle' },
  { 'play', function(v)
    for _, m in ipairs(v.moves) do if m.from == 47 and m.promo == 'n' then return m end end
  end },
  { 'wait', 0.1 },
  expect('the pawn became a knight, and its light comes from the promote event', function(v, c, app)
    local p = pieceAt(v, 55)
    local promoted = false
    for _, e in ipairs(app.response.events or {}) do if e.type == 'promote' and e.kind == 'n' then promoted = true end end
    return p and p.kind == 'n' and (promoted or v.phase == 'enemy') and app.screen.sprites[p.id].promotedAt ~= nil
  end),
  { 'quit' },
}
