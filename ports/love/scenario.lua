-- The prepared battles. The data and the helpers come from src/gallery.ts.
local rules = require('rules')

local scenario = {}

local function sq(name) return ('abcdefgh'):find(name:sub(1, 1), 1, true) - 1 + 8 * (tonumber(name:sub(2, 2)) - 1) end
scenario.sq = sq

local TYPE = { K = 'k', Q = 'q', R = 'r', B = 'b', N = 'n' }

-- Reads a list such as "Ke1 Ra1 c2". An item with no letter before the square is a pawn.
local function pieces(list)
  local result = {}
  for item in list:gmatch('%S+') do
    if #item == 3 then result[#result + 1] = { type = TYPE[item:sub(1, 1)], square = sq(item:sub(2)) }
    else result[#result + 1] = { type = 'p', square = sq(item) } end
  end
  return result
end

local function copy(value)
  if type(value) ~= 'table' then return value end
  local result = {}
  for k, v in pairs(value) do result[k] = copy(v) end
  return result
end

-- The meta of the gallery. It has Scout, thus the player can select an enemy piece.
function scenario.meta() return { crowns = 12, best = 3, runs = 4, upgrades = { gold = 2, bishop = 1, scout = 1 } } end

local function run(army, enemy, rest, traits)
  local units = {}
  for i, piece in ipairs(pieces(army)) do units[i] = { id = i, type = piece.type, home = piece.square } end
  local result = {
    floor = 3, gold = 24, army = units, nextId = #units + 1, relics = {},
    enemy = { pieces = pieces(enemy), traits = traits or {} }, phase = 'battle', draft = nil, shop = {},
  }
  for k, v in pairs(rest or {}) do result[k] = v end
  return result
end

local ARMY = 'Ke1 Ra1 Ng1 c2 d2 e2 f2'

-- Both ports of the prototype use the same data, thus the user can compare them.
local builders = {
  -- The main battle: the boss of floor 4, The Warden. The army is the standard army of a new run (baseArmy).
  -- The relics are the first three relics of src/game/relics.ts.
  boss = function()
    return run(ARMY, 'Ke8 e7 d7 Nb8 Bc8 Ra8 Rh8', { floor = 4, relics = { 'forcedMarch', 'backpedal', 'earlyPromo' } },
      { 'kingKnight' })
  end,
  -- The demos of the gallery. Each one is on floor 3 with 24 gold, no relics, and no traits, unless its data tells a different value.
  -- "Victory": Ra1-a8 is checkmate. Interest gives gold, thus its medal flashes.
  mate = function() return run('Ke1 Ra1 c2', 'Kh8 g7 h7', { gold = 27, relics = { 'bounty', 'interest' } }) end,
  -- "Promotion": a7-a8 promotes.
  promo = function() return run('Ke1 a7', 'Kh6 h5') end,
  -- "Check": Ra1-a8 gives check.
  check = function() return run('Ke1 Ra1 c2', 'Ke8 d7 e7 h6') end,
  -- "Defeat": after Ra8-g8, the enemy king captures the last piece of the army.
  defeat = function() return run('Ke1 Ra8', 'Kh8 g7 h7') end,
  -- "Draw": after Ke1xe2, only the kings remain.
  draw = function() return run('Ke1', 'Ka8 e2') end,
}

-- Returns a function that gives a new copy of the prepared run. Each battle of one session has the same enemy.
function scenario.prepare(name)
  local build = assert(builders[name], 'No prepared battle with the name ' .. tostring(name))
  local first = build()
  return function() return copy(first) end
end

return scenario
