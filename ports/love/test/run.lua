-- The proof that the generated Lua rules give the results of the TypeScript rules.
-- The cases come from test/engine.test.ts and test/game.test.ts. Run from ports/love: luajit test/run.lua
local here = arg[0]:match('^(.*)/test/[^/]*$') or '.'
package.path = here .. '/?.lua;' .. here .. '/generated/?.lua;' .. package.path
require('rules_runtime')
local E = require('engine.index')
local battleLib = require('game.battle')
local relics = require('game.relics')
local runLib = require('game.run')
local upgrades = require('game.upgrades')

local passed, failed = 0, 0

local function same(a, b)
  if type(a) ~= 'table' or type(b) ~= 'table' then return a == b end
  for k, v in pairs(a) do if not same(v, b[k]) then return false end end
  for k in pairs(b) do if a[k] == nil then return false end end
  return true
end

local function show(v)
  if type(v) ~= 'table' then return tostring(v) end
  local keys, parts = {}, {}
  for k in pairs(v) do keys[#keys + 1] = k end
  table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
  for _, k in ipairs(keys) do parts[#parts + 1] = tostring(k) .. '=' .. show(v[k]) end
  return '{' .. table.concat(parts, ',') .. '}'
end

local function eq(actual, expected, what)
  if not same(actual, expected) then
    error(('%s: expected %s, got %s'):format(what or 'value', show(expected), show(actual)), 2)
  end
end

local function test(name, body)
  local start = os.clock()
  local ok, err = pcall(body)
  if ok then passed = passed + 1 else failed = failed + 1 end
  print(('%s %s (%.0f ms)%s'):format(ok and 'ok  ' or 'FAIL', name, (os.clock() - start) * 1000, ok and '' or '\n     ' .. tostring(err)))
end

-- The helpers of test/helpers.ts.
local function sq(name) return ('abcdefgh'):find(name:sub(1, 1), 1, true) - 1 + 8 * (tonumber(name:sub(2, 2)) - 1) end

local function fromFen(fen, turn, rules)
  local pieces, row = {}, 0
  for text in fen:gmatch('[^/]+') do
    local f, r = 0, 7 - row
    for ch in text:gmatch('.') do
      if ch:match('%d') then
        f = f + tonumber(ch)
      else
        local color, kind = ch == ch:upper() and 'w' or 'b', ch:lower()
        local moved = kind == 'p' and r ~= (color == 'w' and 1 or 6)
        pieces[#pieces + 1] = { id = #pieces, type = kind, color = color, square = r * 8 + f, moved = moved }
        f = f + 1
      end
    end
    row = row + 1
  end
  local state = E.createState(pieces, rules)
  state.turn = turn or 'w'
  return state
end

local function perft(state, depth)
  if depth == 0 then return 1 end
  local nodes = 0
  for _, m in ipairs(E.legalMoves(state)) do
    local undo = E.makeMove(state, m)
    nodes = nodes + perft(state, depth - 1)
    E.unmakeMove(state, m, undo)
  end
  return nodes
end

local function sorted(list) table.sort(list) return list end
local function targets(state, from)
  local to = {}
  for _, m in ipairs(E.legalMoves(state)) do if m.from == sq(from) then to[#to + 1] = m.to end end
  return sorted(to)
end
local function scouted(state, from)
  local to = {}
  for _, m in ipairs(E.movesFrom(state, sq(from))) do to[#to + 1] = m.to end
  return sorted(to)
end
local function squares(...)
  local list = {}
  for _, name in ipairs({ ... }) do list[#list + 1] = sq(name) end
  return sorted(list)
end
local function has(list, value)
  for _, v in ipairs(list) do if v == value then return true end end
  return false
end

local START = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR'
local KIWIPETE = 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R'

-- test/engine.test.ts

test('perft from the start position', function()
  eq(perft(fromFen(START), 3), 8902)
  eq(perft(fromFen(START), 4), 197281)
end)

test('perft with castling, en passant, and promotion (Kiwipete)', function()
  eq(perft(fromFen(KIWIPETE), 2), 2039)
  eq(perft(fromFen(KIWIPETE), 3), 97862)
end)

test('unmakeMove restores the state', function()
  local state = fromFen(KIWIPETE)
  local before = show(state)
  perft(state, 3)
  eq(show(state), before)
end)

test('forcedMarch lets a moved pawn move two squares', function()
  local fen = '4k3/7p/8/8/8/4P3/8/4K3'
  eq(targets(fromFen(fen), 'e3'), squares('e4'))
  eq(targets(fromFen(fen, 'w', { w = { forcedMarch = true } }), 'e3'), squares('e4', 'e5'))
end)

test('backpedal lets a pawn move backward to an empty square', function()
  eq(targets(fromFen('4k3/7p/8/8/8/4P3/8/4K3', 'w', { w = { backpedal = true } }), 'e3'), squares('e2', 'e4'))
end)

test('earlyPromo promotes on the seventh rank', function()
  local state = fromFen('4k3/7p/4P3/8/8/8/8/4K3', 'w', { w = { earlyPromo = true } })
  local count = 0
  for _, m in ipairs(E.legalMoves(state)) do
    if m.from == sq('e6') then
      count = count + 1
      assert(m.promo and m.to == sq('e7'))
    end
  end
  eq(count, 4)
end)

test('kingKnight lets the king move and give check as a knight', function()
  local rules = { w = { kingKnight = true } }
  assert(has(targets(fromFen('4k3/7p/8/8/8/8/8/4K2P', 'w', rules), 'e1'), sq('f3')))
  eq(E.inCheck(fromFen('4k3/7p/3K4/8/8/8/8/7P', 'b', rules), 'b'), true)
end)

test('longLeap adds the long knight jump', function()
  eq(targets(fromFen('4k3/7p/8/8/8/8/8/N3K3', 'w', { w = { longLeap = true } }), 'a1'), squares('b3', 'c2', 'b4', 'd2'))
end)

test('sidestep moves a bishop one square without a capture', function()
  local state = fromFen('4k3/8/8/8/8/p7/P7/B3K3', 'w', { w = { sidestep = true } })
  assert(has(targets(state, 'a1'), sq('b1')))
  assert(not has(targets(state, 'a1'), sq('a2')))
end)

test('movesFrom gives the moves of a piece of the side that does not have the move', function()
  local leap = fromFen('4k3/8/8/3n4/8/8/8/4K3', 'w', { b = { longLeap = true } })
  eq(#scouted(leap, 'd5'), 15)
  assert(has(scouted(leap, 'd5'), sq('a4')))
  eq(scouted(leap, 'a1'), {})
  eq(scouted(fromFen('4k3/4r3/8/8/8/8/8/4RK2'), 'e7'), squares('e6', 'e5', 'e4', 'e3', 'e2', 'e1'))
  local state = fromFen('4k3/8/8/3pP3/8/8/8/4K3')
  state.ep = sq('d6')
  eq(scouted(state, 'd5'), squares('d4'))
  eq(state.turn, 'w')
  eq(state.ep, sq('d6'))
  eq(scouted(state, 'e5'), squares('d6', 'e6'))
end)

test('outcome finds checkmate, stalemate, and a lone king', function()
  eq(E.outcome(fromFen('R5k1/5ppp/8/8/8/8/8/4K3', 'b')), { winner = 'w', reason = 'checkmate' })
  eq(E.outcome(fromFen('7k/5Q2/8/8/8/8/8/4K2p', 'b')), { winner = 'w', reason = 'stalemate' })
  eq(E.outcome(fromFen('7k/8/8/8/8/8/8/4K2P', 'b')), { winner = 'w', reason = 'rout' })
  eq(E.outcome(fromFen(START)), nil)
end)

test('the AI captures a free queen and finds mate in one', function()
  local level = { depth = 2, noise = 0 }
  eq(E.chooseMove(fromFen('4k3/8/8/3q4/8/4N3/PPP5/4K3'), level).to, sq('d5'))
  eq(E.chooseMove(fromFen('6k1/5ppp/8/8/8/8/1P6/R3K3'), level).to, sq('a8'))
end)

test('the AI answers in a full position at depth 3 in less than 3 seconds', function()
  local state = fromFen('r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b')
  local start = os.clock()
  assert(E.chooseMove(state, { depth = 3, noise = 0 }))
  local seconds = os.clock() - start
  print(('     depth 3 search: %.0f ms'):format(seconds * 1000))
  assert(seconds < 3)
end)

-- The moves that the TypeScript AI selects under Bun with no noise: { position, side to move, depth, from, to }.
-- The Lua AI must select the same moves. This needs the same sequence of the moves, thus the sort must keep
-- the sequence of equal items (see rules_runtime.lua).
local AI_MOVES = {
  { 'r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b', 1, 45, 28 },
  { 'r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b', 2, 45, 28 },
  { 'r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b', 3, 45, 28 },
  { 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR', 'w', 1, 8, 24 },
  { 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR', 'w', 2, 8, 24 },
  { 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR', 'w', 3, 8, 24 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'w', 1, 12, 40 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'w', 2, 12, 40 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'w', 3, 12, 40 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'b', 1, 25, 18 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'b', 2, 25, 18 },
  { 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R', 'b', 3, 25, 18 },
  { 'rnbqkbnr/pppppppp/8/8/8/2P5/2PPPP2/R3K1N1', 'b', 1, 48, 32 },
  { 'rnbqkbnr/pppppppp/8/8/8/2P5/2PPPP2/R3K1N1', 'b', 2, 48, 32 },
  { 'rnbqkbnr/pppppppp/8/8/8/2P5/2PPPP2/R3K1N1', 'b', 3, 48, 32 },
  { 'rnbqkbnr/pppp1ppp/8/4p3/3P4/8/2P1PP2/R3K1N1', 'b', 1, 36, 27 },
  { 'rnbqkbnr/pppp1ppp/8/4p3/3P4/8/2P1PP2/R3K1N1', 'b', 2, 36, 27 },
  { 'rnbqkbnr/pppp1ppp/8/4p3/3P4/8/2P1PP2/R3K1N1', 'b', 3, 36, 27 },
  { '4k3/8/8/3q4/8/4N3/PPP5/4K3', 'w', 1, 20, 35 },
  { '4k3/8/8/3q4/8/4N3/PPP5/4K3', 'w', 2, 20, 35 },
  { '4k3/8/8/3q4/8/4N3/PPP5/4K3', 'w', 3, 20, 35 },
}

test('the AI selects the moves of the TypeScript AI', function()
  for _, case in ipairs(AI_MOVES) do
    local move = E.chooseMove(fromFen(case[1], case[2]), { depth = case[3], noise = 0 })
    eq({ move.from, move.to }, { case[4], case[5] }, case[1] .. ' at depth ' .. case[3])
  end
end)

-- test/game.test.ts

local function emptyMeta() return { crowns = 0, best = 0, runs = 0, upgrades = {} } end

local function runAgainst(pieces, change)
  local run = runLib.newRun(emptyMeta())
  run.enemy = { pieces = pieces, traits = {} }
  if change then change(run) end
  return run
end

local function play(battle, run, from, to)
  for _, m in ipairs(E.legalMoves(battle.state)) do
    if m.from == sq(from) and m.to == sq(to) then return battleLib.playMove(battle, run, m) end
  end
  error(from .. '-' .. to .. ' is not a legal move')
end

local KING = { type = 'k', square = sq('e8') }

test('the standard army is Ke1, Ra1, Ng1, and pawns on c2, d2, e2, f2', function()
  local army = {}
  for _, unit in ipairs(runLib.newRun(emptyMeta()).army) do army[unit.id] = unit.type .. unit.home end
  eq(army, { 'k4', 'r0', 'n6', 'p10', 'p11', 'p12', 'p13' })
end)

test('a win gives gold', function()
  local run = runAgainst({ KING, { type = 'p', square = sq('a2') } })
  local battle = battleLib.createBattle(run)
  play(battle, run, 'a1', 'a2')
  eq(battle.result, { winner = 'w', reason = 'rout', reward = { captures = 1, clear = 4, bonuses = {} } })
  eq(battleLib.totalGold(battle.result.reward), 5)
end)

test('Bounty and Interest add gold', function()
  local run = runAgainst({ KING, { type = 'r', square = sq('a2') } }, function(r)
    r.relics = { 'bounty', 'interest' }
    r.gold = 20
  end)
  local battle = battleLib.createBattle(run)
  eq(play(battle, run, 'a1', 'a2'), { capture = { square = sq('a2'), gold = 7.5 }, relics = { 'bounty', 'interest' } })
  eq(battle.result.reward, { captures = 8, clear = 4, bonuses = { { id = 'interest', label = 'Interest', gold = 6 } } })
end)

test('a captured unit leaves the army, and Second Wind returns the first one', function()
  local enemy = { KING, { type = 'r', square = sq('a8') }, { type = 'r', square = sq('h8') } }
  local function lose(run)
    local battle = battleLib.createBattle(run)
    play(battle, run, 'e2', 'e3')
    play(battle, run, 'a8', 'a1')
    play(battle, run, 'e1', 'e2')
    play(battle, run, 'a1', 'g1')
    return battle
  end
  local plain = lose(runAgainst(enemy))
  eq(plain.lost, { 2, 3 })
  eq(plain.rescued, {})
  eq(plain.taken, { w = {}, b = { 'r', 'n' } })
  local battle = lose(runAgainst(enemy, function(r) r.relics = { 'secondWind' } end))
  eq(battle.lost, { 3 })
  eq(battle.rescued, { 2 })
end)

test('Conscription adds a pawn that does not join the army', function()
  local run = runAgainst({ KING, { type = 'p', square = sq('a7') } }, function(r) r.relics = { 'conscription' } end)
  local battle = battleLib.createBattle(run)
  local count = 0
  for s = 0, 63 do
    local p = battle.state.board[s + 1]
    if p and p.id == relics.CONSCRIPT_ID then count = count + 1 end
  end
  eq(count, 1)
  local pawn = battle.state.board[sq('a2') + 1]
  eq({ pawn.id, pawn.type, pawn.color }, { relics.CONSCRIPT_ID, 'p', 'w' })
  eq(#run.army, 7)
end)

test('a relic gives its movement rule to the battle', function()
  local run = runAgainst({ KING, { type = 'p', square = sq('a7') } }, function(r) r.relics = { 'kingKnight' } end)
  run.enemy.traits = { 'forcedMarch' }
  eq(battleLib.createBattle(run).state.rules, { w = { kingKnight = true }, b = { forcedMarch = true } })
end)

-- Cases that the TypeScript tests do not have. They guard the places where Lua differs from JavaScript.

test('rulesFor reads each relic of a list that also has relics with no rule', function()
  eq(relics.rulesFor({ 'bounty', 'forcedMarch', 'interest', 'secondWind', 'longLeap', 'conscription' }), { forcedMarch = true, longLeap = true })
  eq(relics.rulesFor({ 'bounty' }), {})
end)

test('the relic ids and the upgrade ids are complete', function()
  eq(sorted(relics.RELIC_IDS), sorted({ 'forcedMarch', 'backpedal', 'earlyPromo', 'kingKnight', 'longLeap', 'sidestep', 'bounty', 'secondWind', 'conscription', 'interest' }))
  eq(upgrades.canScout(emptyMeta()), false)
  eq(upgrades.canScout({ crowns = 12, best = 3, runs = 4, upgrades = { gold = 2, bishop = 1, scout = 1 } }), true)
end)

test('a draw by the clock and a battle with only the kings', function()
  local state = fromFen('4k3/7p/8/8/8/8/8/R3K3')
  state.clock = 100
  eq(E.outcome(state), { winner = nil, reason = 'clock' })
  eq(E.outcome(fromFen('4k3/8/8/8/8/8/8/4K3')), { winner = nil, reason = 'bare' })
end)

print(('%d passed, %d failed'):format(passed, failed))
os.exit(failed == 0 and 0 or 1)
