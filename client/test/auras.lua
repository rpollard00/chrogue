--[[
  The auras of a battle: the badge of a piece that has a boon, and the zone of an aura in focus (aura.lua).
  - The badges are on the squares that the view gives, and only there. A battle with no such relic has no badge.
  - At the start of a battle, each badge comes into view one more time after the banner, and the medals of their relics
    flash. The battle is not settled until then.
  - A piece that loses a boon loses its badge, and a piece that gets a boon gets one. The battle is not settled during the motion.
  - The pointer on a medal, on a source piece, or on a piece with a badge puts the aura in focus: the board shows its
    zone. The pointer on a piece also lights the medals of its relics, and a selected piece does the same.
  - The largest content: 16 white and 16 black pieces with badges, a check, a selection with a capture ring on a badge,
    and the last move.
  Run with: --size 1440x900 --seed 7 --debug --no-save --script test/auras.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end
local function relic(id) return { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end

local function sq(name) return ('abcdefgh'):find(name:sub(1, 1), 1, true) - 1 + 8 * (tonumber(name:sub(2, 2)) - 1) end
-- 'Ke1 Bd2' as a list of pieces. `key` is the name of the field for the square: `home` or `square`.
local function pieces(text, key)
  local list = {}
  for kind, name in text:gmatch('(%a)(%a%d)') do list[#list + 1] = { kind = kind:lower(), [key] = sq(name) } end
  return list
end
local function army(text) return { 'send', { cmd = 'debug_set_army', units = pieces(text, 'home') } } end
local function enemy(text, traits) return { 'send', { cmd = 'debug_set_enemy', pieces = pieces(text, 'square'), traits = traits or {} } } end
local function hover(name) return { 'hover', 'control', 'square', sq(name) } end

local function same(a, b)
  if type(a) ~= 'table' or type(b) ~= 'table' then return a == b end
  for k, v in pairs(a) do if not same(v, b[k]) then return false end end
  for k in pairs(b) do if a[k] == nil then return false end end
  return true
end
-- True if the text of `drawn` has this badge. '3:shield' is not in '13:shield'.
local function hasBadge(text, badge) return (' ' .. text .. ' '):find(' ' .. badge .. ' ', 1, true) ~= nil end
-- The phases of the first boon of each badge are all in `phases`, and the number of the badges.
local function allIn(client, phases)
  for _, badge in ipairs(client.auras.badges) do
    if not phases[badge.boons[1].phase] then return false, #client.auras.badges end
  end
  return true, #client.auras.badges
end
local function flashed(client) return table.concat(client.flashed.player, ' ') .. '|' .. table.concat(client.flashed.enemy, ' ') end

-- The badges of the client as text: 'c2:shield d1:shield+moves'. With `phases`, each boon has its phase.
local function drawn(client, phases)
  local parts = {}
  for _, badge in ipairs(client.auras.badges) do
    local boons = {}
    for i, b in ipairs(badge.boons) do boons[i] = phases and (b.boon .. '=' .. b.phase) or b.boon end
    parts[#parts + 1] = badge.square .. ':' .. table.concat(boons, '+')
  end
  return table.concat(parts, ' ')
end

-- The boons of the pieces of the view, as the same text.
local function given(view)
  local list = {}
  for _, p in ipairs(view.pieces) do
    if #p.auras > 0 then
      local boons = {}
      for i, entry in ipairs(p.auras) do boons[i] = entry.boon end
      list[#list + 1] = { square = p.square, text = p.square .. ':' .. table.concat(boons, '+') }
    end
  end
  table.sort(list, function(a, b) return a.square < b.square end)
  for i, item in ipairs(list) do list[i] = item.text end
  return table.concat(list, ' ')
end

local function atRest(client)
  for _, badge in ipairs(client.auras.badges) do
    for _, b in ipairs(badge.boons) do if b.phase ~= 'rest' then return false end end
  end
  return true
end

-- The badges agree with the view, and no badge moves.
local function agrees(view, client) return drawn(client) == given(view) and atRest(client), drawn(client, true) .. ' / ' .. given(view) end

-- The auras in focus as text: 'blessing/w/shield divineRight/w/moves'.
local function focus(client)
  local parts = {}
  for i, item in ipairs(client.auras.focus) do parts[i] = item.relic .. '/' .. item.color .. '/' .. item.boon end
  return table.concat(parts, ' ')
end
local function lit(client) return table.concat(client.auras.lit.player, ' ') .. '|' .. table.concat(client.auras.lit.enemy, ' ') end
local function hoverState(client) return focus(client) .. ' lit ' .. lit(client) end

local function auraOf(view, id, color)
  for _, item in ipairs(view.auras) do if item.relic == id and item.color == color then return item end end
end
local function pieceAt(view, s)
  for _, p in ipairs(view.pieces) do if p.square == s then return p end end
end

-- The largest content: each white piece but the king has a shield, and the two bishops next to the king have two badges.
local FULL_ARMY = 'Ba1 Nb1 Bc1 Bd1 Ke1 Bf1 Ng1 Bh1 Na2 Bb2 Bc2 Pd2 Pe2 Pf2 Bg2 Ph2'
local FULL_ENEMY = 'Ka5 Ra8 Bb8 Nc8 Qd8 Re8 Nf8 Bg8 Pa7 Pb7 Bc7 Pd7 Be7 Pf7 Pg7 Ph7'
-- The badges of the first position: a1, c1, a2 and c2, around the bishop on b2.
local AROUND = '0:shield 2:shield 8:shield 10:shield'

return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' }, { 'settle' },
  expect('a battle with no relic has no badge and no aura', function(v, c)
    return #v.auras == 0 and #c.auras.badges == 0 and focus(c) == '' and given(v) == ''
  end),

  -- Blessing only. The bishop on b2 protects four pieces. Two of them are on the a-file, next to the frame of the board.
  relic('blessing'), army('Kh1 Bb2 Na1 Pa2 Rc1 Pc2'), enemy('Ke8 Pa7'), { 'response' },
  expect('at the start of a battle with badges, the battle is not settled before the badges came', function(v, c, app)
    return c.startPop and drawn(c) == AROUND and atRest(c) and not app.screen:settled() and flashed(c) == '|', drawn(c, true)
  end),
  { 'settle' },
  expect('after the banner, the badges came one more time and the medal of Blessing flashed', function(v, c)
    return not c.startPop and flashed(c) == 'blessing|' and agrees(v, c), drawn(c, true) .. ' flashed ' .. flashed(c)
  end),
  { 'hover', 'none' }, shot('auras-blessing'),
  hover('b2'), { 'wait', 0.3 },
  expect('the pointer on a source bishop with no badge: its aura is in focus and its medal is lit', function(v, c)
    return #pieceAt(v, sq('b2')).auras == 0 and focus(c) == 'blessing/w/shield' and lit(c) == 'blessing|', hoverState(c)
  end),
  expect('the square of the source is in the zone', function(v, c)
    local zone = c.auras.focus[1].zone
    return same(zone, auraOf(v, 'blessing', 'w').zone) and same(zone, { 0, 1, 2, 8, 9, 10, 16, 17, 18 }), table.concat(zone, ' ')
  end),
  shot('auras-source'),
  { 'hover', 'none' }, { 'wait', 0.3 },

  -- The bishop goes away: the four pieces lose their badges.
  { 'click', 'b2' }, { 'click', 'e5' }, { 'response' },
  expect('the badges go away, and the battle is not settled', function(v, c, app)
    local ok, count = allIn(c, { loss = true })
    return ok and count == 4 and given(v) == '' and not app.screen:settled(), drawn(c, true)
  end),
  { 'wait', 0.1 }, shot('auras-loss'),
  { 'settle' },
  expect('the pieces that the bishop left have no badge', function(v, c) return drawn(c) == '' and given(v) == '', drawn(c, true) end),
  -- The bishop comes back: the four pieces get their badges after the move.
  { 'click', 'e5' }, { 'click', 'b2' }, { 'response' },
  expect('the new badges come into view, and the battle is not settled', function(v, c, app)
    local ok, count = allIn(c, { wait = true, gain = true })
    return ok and count == 4 and given(v) == AROUND and not app.screen:settled(), drawn(c, true)
  end),
  -- The start of the motion of a badge on the a-file, and its ring near its largest size.
  { 'wait', 0.17 }, shot('auras-gain'), { 'wait', 0.2 }, shot('auras-gain-ring'),
  { 'settle' },
  expect('the four pieces have their badges again', function(v, c)
    local ok, detail = agrees(v, c)
    return ok and drawn(c) == AROUND, detail
  end),

  -- The two relics on the two sides.
  relic('divineRight'),
  army('Ke1 Bd1 Bd2 Pc2 Pe2 Nb1 Ra1 Pg2 Ph2'),
  enemy('Ke8 Bd8 Be7 Pd7 Pf7 Nf6 Ra8 Pa7 Ph7', { 'blessing', 'divineRight' }),
  { 'settle' },
  expect('the badges of the two sides agree with the view, and the medals of the two fans flashed', function(v, c)
    local ok, detail = agrees(v, c)
    return ok and #c.auras.badges == 9 and hasBadge(drawn(c), '3:shield+moves') and hasBadge(drawn(c), '59:shield+moves')
      and not c.startPop and flashed(c) == 'blessing divineRight|blessing divineRight', detail .. ' flashed ' .. flashed(c)
  end),
  { 'hover', 'none' }, { 'wait', 0.3 },
  expect('with no pointer, no aura is in focus and no medal is lit', function(v, c) return hoverState(c) == ' lit |', hoverState(c) end),
  shot('auras-rest'), dump('auras-rest'),

  { 'hover', 'medal', 'player', 1 }, { 'wait', 0.4 },
  expect('the pointer on the Blessing medal: the Blessing aura of the player is in focus with the zone of the view', function(v, c)
    local item = c.auras.focus[1]
    return focus(c) == 'blessing/w/shield' and same(item.zone, auraOf(v, 'blessing', 'w').zone) and #item.zone > 0
      and lit(c) == '|', hoverState(c)
  end),
  shot('auras-medal'),
  { 'hover', 'medal', 'enemy', 2 }, { 'wait', 0.4 },
  expect('the pointer on a trait medal: the aura of the enemy is in focus', function(v, c)
    return focus(c) == 'divineRight/b/moves' and same(c.auras.focus[1].zone, auraOf(v, 'divineRight', 'b').zone), hoverState(c)
  end),
  shot('auras-trait-medal'),
  { 'hover', 'none' }, { 'wait', 0.05 },
  expect('the pointer away: no aura is in focus', function(v, c) return hoverState(c) == ' lit |', hoverState(c) end),
  hover('c2'), { 'wait', 0.3 },
  expect('the pointer on a piece with a badge: the aura of its relic is in focus, and the medal is lit', function(v, c)
    return focus(c) == 'blessing/w/shield' and lit(c) == 'blessing|', hoverState(c)
  end),
  shot('auras-badged-piece'),
  hover('d2'), { 'wait', 0.3 },
  expect('the pointer on a bishop with two badges: the two auras are in focus', function(v, c)
    return focus(c) == 'blessing/w/shield divineRight/w/moves' and lit(c) == 'blessing divineRight|', hoverState(c)
  end),
  shot('auras-two-auras'),
  hover('e1'), { 'wait', 0.3 },
  expect('the pointer on the king: the aura that it is the source of is in focus', function(v, c)
    return focus(c) == 'divineRight/w/moves' and lit(c) == 'divineRight|', hoverState(c)
  end),
  hover('d8'), { 'wait', 0.3 },
  expect('the pointer on an enemy bishop: the auras of the traits are in focus, and the trait medals are lit', function(v, c)
    return focus(c) == 'blessing/b/shield divineRight/b/moves' and lit(c) == '|blessing divineRight', hoverState(c)
  end),
  shot('auras-enemy-bishop'),
  hover('a1'), { 'wait', 0.3 },
  expect('the pointer on a piece with no badge: no aura is in focus', function(v, c) return hoverState(c) == ' lit |', hoverState(c) end),
  { 'click', 'c2' }, { 'hover', 'none' }, { 'wait', 0.3 },
  expect('a selected piece with a badge lights the medal, and no zone shows', function(v, c)
    return c.selected == sq('c2') and focus(c) == '' and lit(c) == 'blessing|', hoverState(c)
  end),
  shot('auras-selected'),
  { 'key', 'escape' },

  -- The largest content. The pawn opens the diagonal to the king, and the enemy bishop gives check next to its king.
  army(FULL_ARMY), enemy(FULL_ENEMY, { 'blessing', 'divineRight' }), { 'settle' },
  expect('32 pieces: each white piece but the king has a shield', function(v, c)
    local ok, detail = agrees(v, c)
    local shields = 0
    for _, badge in ipairs(c.auras.badges) do
      if badge.square < 16 and badge.boons[1].boon == 'shield' then shields = shields + 1 end
    end
    return ok and #v.pieces == 32 and shields == 15, detail
  end),
  -- The core answers the requests in their order, thus the enemy move of the script is before the move of the AI.
  { 'click', 'd2' }, { 'click', 'd3' },
  { 'send', { cmd = 'debug_enemy_move', from = sq('e7'), to = sq('b4') } }, { 'settle' },
  { 'click', 'a2' }, { 'hover', 'none' }, { 'wait', 0.8 },
  expect('a check, the last move, and a capture ring on a piece with a badge', function(v, c)
    local bishop = pieceAt(v, sq('b4'))
    local capture = false
    for _, m in ipairs(v.moves) do capture = capture or (m.from == sq('a2') and m.to == sq('b4') and m.capture) end
    local ok, detail = agrees(v, c)
    return ok and v.check == sq('e1') and v.last.to == sq('b4') and c.selected == sq('a2') and capture
      and #bishop.auras == 1 and bishop.auras[1].boon == 'moves' and #v.pieces == 32, detail
  end),
  shot('auras-largest'), dump('auras-largest'),
  hover('b4'), { 'wait', 0.3 },
  expect('the pointer on the enemy bishop next to its king: the two auras of the enemy are in focus', function(v, c)
    return focus(c) == 'blessing/b/shield divineRight/b/moves', hoverState(c)
  end),
  shot('auras-largest-focus'),
  { 'quit' },
}
