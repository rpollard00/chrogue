-- The icon set: each icon is a line drawing on a grid of 24 units, as an SVG path.
-- This module reads the path data and draws the lines. To add an icon, add its path.
local lg = love.graphics

local icons = {}

local PATHS = {
  forcedMarch = 'M6 12l6-6 6 6M6 19l6-6 6 6',
  backpedal = 'M9 14l-4-4 4-4M5 10h9a5 5 0 0 1 0 10h-3',
  earlyPromo = 'M12 3l2.7 5.6 6.1.8-4.5 4.3 1.1 6.1L12 16.9l-5.4 2.9 1.1-6.1-4.5-4.3 6.1-.8z',
  kingKnight = 'M6 20V11a6 6 0 0 1 12 0v9h-4v-9a2 2 0 0 0-4 0v9z',
  longLeap = 'M7 21V7h11M14 3l4 4-4 4',
  sidestep = 'M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3',
  bounty = 'M12 5a7 7 0 1 0 0 14a7 7 0 0 0 0-14M12 2v5M12 17v5M2 12h5M17 12h5',
  secondWind = 'M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5',
  conscription = 'M6 21V4M6 5h12l-3 4 3 4H6',
  interest = 'M3 17l6-6 4 4 8-8M15 7h6v6',
  coins = 'M5 8c0-1.7 3.1-3 7-3s7 1.3 7 3-3.1 3-7 3-7-1.3-7-3zM5 8v4c0 1.7 3.1 3 7 3s7-1.3 7-3V8M5 12v4c0 1.7 3.1 3 7 3s7-1.3 7-3v-4',
  chest = 'M4 19v-9a4 4 0 0 1 4-4h8a4 4 0 0 1 4 4v9zM4 12h16M12 11v4',
  tag = 'M3 12V4h8l10 10-8 8zM7.5 8.5h.01',
  eye = 'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6z',
  vault = 'M3 20v-7h5v3h8v-3h5v7zM5.5 9a8 8 0 0 1 13 0M15 8.5l3.5.5.5-3.5',
  crossfire = 'M5 19L19 5M19 19L5 5M5 10V5h5M14 5h5v5',
  closeQuarters = 'M12 3l-2.5 4v8h5V7zM7 15h10M12 15v6',
  pilgrimLeap = 'M3 15h6v6H3zM15 3h6v6h-6zM6 15a9 9 0 0 1 9-9',
  queenFlight = 'M19 5c-7 0-12 4-12 12c7 0 12-5 12-12zM4 20L14 10',
  gallop = 'M4 7h7M4 12h5M4 17h7M14 6l6 6-6 6',
  crusade = 'M12 3l8 3v6c0 5-3.5 8-8 9c-4.5-1-8-4-8-9V6zM12 8v8M8.5 11h7',
  royalMarch = 'M5 12h14M5 12L4 4l4.5 3.5L12 3l3.5 4.5L20 4l-1 8M5 18h13M15 15l3 3-3 3',
  huntress = 'M8 3a10.5 10.5 0 0 1 0 18M8 3v18M3 12h18M18 9l3 3-3 3',
  apprenticeship = 'M9 4a3 3 0 1 0 0 6a3 3 0 0 0 0-6M5 20c0-4 2-6 4-9c2 3 4 5 4 9zM18 5v6M15 8h6',
  coup = 'M13 2L5 13h6l-1 9l8-12h-6z',
  gambit = 'M12 4v16M7 20h10M5 7h14M5 7l-3 7h6zM19 7l-3 7h6z',
  shieldWall = 'M3 10h6v6l-3 4-3-4zM9 10h6v6l-3 4-3-4zM15 10h6v6l-3 4-3-4zM12 7V2M9.5 4.5L12 2l2.5 2.5',
  enfilade = 'M4 18h12V7M12 11l4-4 4 4',
  echelon = 'M12 21v-6M12 15L5 5M12 15l7-10M5 10V5h5M14 5h5v5',
  gem = 'M6 4h12l4 6-10 11L2 10zM2 10h20',
  vase = 'M9 3h6M10 3c0 3-4 5-4 10a6 6 0 0 0 12 0c0-5-4-7-4-10M12 19v2M9 21h6',
  die = 'M5 5h14v14H5zM9 9h.01M15 9h.01M12 12h.01M9 15h.01M15 15h.01',
  letter = 'M3 6h18v12H3zM3 6l9 7 9-7',
  troops = 'M9 5a3 3 0 1 0 0 6a3 3 0 0 0 0-6M3 20v-1a6 6 0 0 1 12 0v1M16 5.2a3 3 0 0 1 0 5.6M18 14a6 6 0 0 1 3 5v1',
  frame = 'M4 4h16v16H4zM12 8l4 4-4 4-4-4z',
}

-- The mark of an id that has no path: a plain ring. Content of the core can come before its icon.
local FALLBACK = 'M12 6a6 6 0 1 0 0 12a6 6 0 0 0 0-12'

-- The art of each upgrade: a piece, or an icon.
icons.UPGRADE_ART = {
  pawn = { kind = 'piece', type = 'p' },
  gold = { kind = 'icon', id = 'chest' },
  bishop = { kind = 'piece', type = 'b' },
  haggle = { kind = 'icon', id = 'tag' },
  scout = { kind = 'icon', id = 'eye' },
  knight = { kind = 'piece', type = 'n' },
  heirloom = { kind = 'icon', id = 'gem' },
  antiquary = { kind = 'icon', id = 'vase' },
  fixer = { kind = 'icon', id = 'die' },
  envoy = { kind = 'icon', id = 'letter' },
  muster = { kind = 'icon', id = 'troops' },
  curator = { kind = 'icon', id = 'frame' },
}

-- Reads the numbers and the command letters of a path. A number can start with '-' or '.' with no space before it.
local function tokens(path)
  local list, i = {}, 1
  while i <= #path do
    local ch = path:sub(i, i)
    if ch:match('%a') then
      list[#list + 1] = ch
      i = i + 1
    elseif ch:match('[%d%.%-]') then
      local number = path:match('^%-?%d*%.?%d+', i)
      list[#list + 1] = tonumber(number)
      i = i + #number
    else
      i = i + 1
    end
  end
  return list
end

-- The points of an elliptical arc, from the SVG endpoint form. The icons use circles only (rx = ry, no rotation).
local function arc(points, x1, y1, r, large, sweep, x2, y2)
  local dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
  local d2 = dx * dx + dy * dy
  if d2 == 0 then return end
  r = math.max(r, math.sqrt(d2))
  local k = math.sqrt(math.max(0, r * r - d2) / d2)
  if large == sweep then k = -k end
  local cx, cy = (x1 + x2) / 2 + k * dy, (y1 + y2) / 2 - k * dx
  local a1, a2 = math.atan2(y1 - cy, x1 - cx), math.atan2(y2 - cy, x2 - cx)
  local delta = a2 - a1
  if sweep == 1 and delta < 0 then delta = delta + 2 * math.pi end
  if sweep == 0 and delta > 0 then delta = delta - 2 * math.pi end
  local steps = math.max(4, math.ceil(math.abs(delta) / (math.pi / 16)))
  for i = 1, steps do
    local a = a1 + delta * i / steps
    points[#points + 1] = cx + math.cos(a) * r
    points[#points + 1] = cy + math.sin(a) * r
  end
end

local ARGS = { M = 2, L = 2, H = 1, V = 1, A = 7, C = 6, S = 4, Z = 0 }

-- The points of a cubic Bézier curve from the current point.
local function cubic(points, x0, y0, x1, y1, x2, y2, x3, y3)
  for i = 1, 12 do
    local t = i / 12
    local u = 1 - t
    points[#points + 1] = u * u * u * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x3
    points[#points + 1] = u * u * u * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y3
  end
end

-- Returns the lines of a path. Each line is a list of x, y values.
local function parse(path)
  local list, lines, points = tokens(path), {}, nil
  local x, y, startX, startY = 0, 0, 0, 0
  -- The second control point of the last curve, for the reflection of S.
  local lastCX, lastCY = nil, nil
  local i, command = 1, nil
  while i <= #list do
    if type(list[i]) == 'string' then
      command = list[i]
      i = i + 1
    end
    local upper = command:upper()
    local relative = command ~= upper
    local count = assert(ARGS[upper], 'The icon path has a command that this module does not read: ' .. command)
    local a = {}
    for n = 1, count do a[n] = list[i + n - 1] end
    i = i + count
    if upper == 'M' then
      x, y = (relative and x or 0) + a[1], (relative and y or 0) + a[2]
      startX, startY = x, y
      points = { x, y }
      lines[#lines + 1] = points
      -- The pairs after the first pair of a move are lines.
      command = relative and 'l' or 'L'
    elseif upper == 'Z' then
      points[#points + 1] = startX
      points[#points + 1] = startY
      x, y = startX, startY
    else
      local nx, ny = x, y
      if upper == 'L' then nx, ny = (relative and x or 0) + a[1], (relative and y or 0) + a[2]
      elseif upper == 'H' then nx = (relative and x or 0) + a[1]
      elseif upper == 'V' then ny = (relative and y or 0) + a[1]
      elseif upper == 'A' then nx, ny = (relative and x or 0) + a[6], (relative and y or 0) + a[7]
      elseif upper == 'C' then nx, ny = (relative and x or 0) + a[5], (relative and y or 0) + a[6]
      elseif upper == 'S' then nx, ny = (relative and x or 0) + a[3], (relative and y or 0) + a[4] end
      local ox, oy = relative and x or 0, relative and y or 0
      if upper == 'A' then
        arc(points, x, y, a[1], a[4], a[5], nx, ny)
      elseif upper == 'C' then
        cubic(points, x, y, ox + a[1], oy + a[2], ox + a[3], oy + a[4], nx, ny)
        lastCX, lastCY = ox + a[3], oy + a[4]
      elseif upper == 'S' then
        local c1x, c1y = x, y
        if lastCX then c1x, c1y = 2 * x - lastCX, 2 * y - lastCY end
        cubic(points, x, y, c1x, c1y, ox + a[1], oy + a[2], nx, ny)
        lastCX, lastCY = ox + a[1], oy + a[2]
      else
        points[#points + 1] = nx
        points[#points + 1] = ny
      end
      if upper ~= 'C' and upper ~= 'S' then lastCX = nil end
      x, y = nx, ny
    end
    if upper == 'M' or upper == 'Z' then lastCX = nil end
  end
  return lines
end

icons.PATHS = PATHS
icons.parse = parse

local parsed = {}

-- Draws an icon with its center at a point. `size` is the width of the 24 unit grid. The line has a width of 2 grid units
-- and round ends. An id with no path gets the plain ring. Set the color before the call.
function icons.draw(id, cx, cy, size)
  local lines = parsed[id]
  if not lines then
    lines = parse(PATHS[id] or FALLBACK)
    parsed[id] = lines
  end
  local scale = size / 24
  lg.push()
  lg.translate(cx - size / 2, cy - size / 2)
  lg.scale(scale)
  lg.setLineWidth(2)
  lg.setLineJoin('none')
  for _, points in ipairs(lines) do
    lg.line(points)
    for n = 1, #points, 2 do lg.circle('fill', points[n], points[n + 1], 1, 12) end
  end
  lg.setLineJoin('miter')
  lg.pop()
end

local crown
-- The crown of the crowns: a solid shape.
function icons.crown(cx, cy, size)
  crown = crown or love.math.triangulate(4, 18, 20, 18, 21, 8, 16, 12, 12, 5, 8, 12, 3, 8)
  lg.push()
  lg.translate(cx - size / 2, cy - size / 2)
  lg.scale(size / 24)
  for _, t in ipairs(crown) do lg.polygon('fill', t) end
  lg.pop()
end

-- The icon of a currency: 'gold' or 'crowns'. Set the color before the call.
function icons.currency(currency, cx, cy, size)
  if currency == 'crowns' then icons.crown(cx, cy, size) else icons.coin(cx, cy, size) end
end

-- The coin of the purse: a ring and a disk.
function icons.coin(cx, cy, size)
  local unit = size / 24
  lg.setLineWidth(3 * unit)
  lg.circle('line', cx, cy, 8 * unit, 40)
  lg.circle('fill', cx, cy, 5 * unit, 40)
end

return icons
