--[[
  The barred ring: the mark of a capture that a shield refuses (`denied` and `enemy_denied` of the view).
  - The enemy has the trait Blessing. A selected rook attacks two pieces with a shield: each square has a barred ring,
    the square is not a target, the shield badge of the piece pulses, and the trait medal is lit.
  - Escape removes the marks. A click on a barred square is a click on an enemy piece: it clears the selection.
  - With Scout and Blessing, a selected enemy rook shows the barred ring on a piece of the player.
  Run with: --size 1440x900 --seed 7 --debug --no-save --script test/denied.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end
local function expect(label, check) return { 'expect', check, label } end

local function sq(name) return ('abcdefgh'):find(name:sub(1, 1), 1, true) - 1 + 8 * (tonumber(name:sub(2, 2)) - 1) end
-- 'Ke1 Bd2' as a list of pieces. `key` is the name of the field for the square: `home` or `square`.
local function pieces(text, key)
  local list = {}
  for kind, name in text:gmatch('(%a)(%a%d)') do list[#list + 1] = { kind = kind:lower(), [key] = sq(name) } end
  return list
end
local function army(text) return { 'send', { cmd = 'debug_set_army', units = pieces(text, 'home') } } end
local function enemy(text, traits) return { 'send', { cmd = 'debug_set_enemy', pieces = pieces(text, 'square'), traits = traits or {} } } end

local function list(t) return table.concat(t, ' ') end
local function has(t, value)
  for _, v in ipairs(t) do if v == value then return true end end
  return false
end
local function lit(client) return list(client.auras.lit.player) .. '|' .. list(client.auras.lit.enemy) end
-- The marks of the selection as text, for the detail of a step.
local function marks(client)
  return ('selected %d denied %s targets %s pulse %s lit %s'):format(client.selected, list(client.denied), list(client.targets),
    list(client.auras.pulse), lit(client))
end
local function count(sent, cmd)
  local n = 0
  for _, name in ipairs(sent) do if name == cmd then n = n + 1 end end
  return n
end

return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' }, { 'settle' },
  -- The rook on d1 attacks the pawn on d7 (a light square) and the knight on g1 (a dark square). The two have a shield
  -- from a bishop. The rook can capture the rook on a1.
  army('Kc2 Rd1'), enemy('Ke8 Be7 Pd7 Ra1 Ng1 Bh2', { 'blessing' }), { 'settle' },
  expect('the view has the two denied captures of the rook', function(v)
    return #v.denied == 2 and v.denied[1].from == sq('d1') and v.denied[2].from == sq('d1'), #v.denied
  end),
  expect('with no selection, the board has no barred ring', function(v, c)
    return #c.denied == 0 and #c.auras.pulse == 0 and lit(c) == '|', marks(c)
  end),
  { 'click', 'd1' }, { 'hover', 'none' }, { 'settle' },
  expect('the selected rook has a barred ring on g1 and on d7, and these squares are not targets', function(v, c)
    return c.selected == sq('d1') and list(c.denied) == sq('g1') .. ' ' .. sq('d7') and not has(c.targets, sq('g1'))
      and not has(c.targets, sq('d7')) and has(c.targets, sq('a1')) and has(c.targets, sq('d6')), marks(c)
  end),
  expect('the shield badges of the two pieces pulse, the trait medal is lit, and no zone shows', function(v, c, app)
    return list(c.auras.pulse) == sq('g1') .. ' ' .. sq('d7') and lit(c) == '|blessing' and #c.auras.focus == 0
      and app.screen:settled(), marks(c)
  end),
  { 'wait', 0.3 }, shot('denied-rook'), dump('denied-rook'),
  { 'key', 'escape' }, { 'settle' },
  expect('after Escape, the board has no barred ring', function(v, c)
    return c.selected == -1 and #c.denied == 0 and #c.auras.pulse == 0 and lit(c) == '|', marks(c)
  end),
  { 'click', 'd1' }, { 'click', 'd7' }, { 'settle' },
  expect('a click on a barred square clears the selection and sends no move', function(v, c, app)
    return c.selected == -1 and #c.denied == 0 and count(app.sent, 'move') == 0 and v.phase == 'player', marks(c)
  end),

  -- Scout. The enemy rook on a2 attacks the pawn on b2, which has a shield from the bishop on c1. It can capture the
  -- knight on a1.
  { 'send', { cmd = 'debug_set_upgrade', upgrade = 'scout', level = 1 } },
  { 'send', { cmd = 'debug_set_relic', relic = 'blessing', on = true } },
  -- The enemy is first: the knight of the army goes to the square of the enemy rook of the first position.
  enemy('Ke8 Ra2'), army('Kh1 Bc1 Pb2 Na1'), { 'settle' },
  expect('with Scout, the view has the denied capture of the enemy rook', function(v)
    return v.scout and #v.denied == 0 and #v.enemy_denied == 1 and v.enemy_denied[1].to == sq('b2'), #v.enemy_denied
  end),
  { 'click', 'a2' }, { 'hover', 'none' }, { 'settle' },
  expect('the selected enemy rook has a barred ring on b2, and the Blessing medal of the player is lit', function(v, c)
    return c.scouting and list(c.denied) == tostring(sq('b2')) and not has(c.targets, sq('b2')) and has(c.targets, sq('a1'))
      and list(c.auras.pulse) == tostring(sq('b2')) and lit(c) == 'blessing|', marks(c)
  end),
  { 'wait', 0.3 }, shot('denied-scout'), dump('denied-scout'),
  { 'key', 'escape' }, { 'settle' },
  expect('after Escape, the board has no barred ring of the enemy rook', function(v, c)
    return #c.denied == 0 and #c.auras.pulse == 0 and lit(c) == '|', marks(c)
  end),
  { 'quit' },
}
