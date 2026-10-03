--[[
  The SVG path reader of icons.lua. Each check is a property of a path that a
  fault of the reader changes: the points stay in the grid of 24 units, the arcs are circles with their radius, a closed
  path ends at its start, and the S command reflects the control point of the curve before it.
  Run with: --no-save --script test/icons.lua
]]
local icons = require('icons')

local function near(a, b, tolerance) return math.abs(a - b) <= (tolerance or 0.02) end
local function distance(x, y, cx, cy) return math.sqrt((x - cx) ^ 2 + (y - cy) ^ 2) end
local function onCircle(points, cx, cy, r, tolerance)
  for i = 1, #points, 2 do
    if not near(distance(points[i], points[i + 1], cx, cy), r, tolerance) then
      return false, ('point %.2f %.2f is not on the circle of radius %g'):format(points[i], points[i + 1], r)
    end
  end
  return true
end

local function check()
  for id in pairs(icons.PATHS) do
    for _, points in ipairs(icons.parse(icons.PATHS[id])) do
      if #points < 4 then return false, id .. ' has a line with one point' end
      for i = 1, #points do
        if points[i] < -0.01 or points[i] > 24.01 then return false, ('%s has a point outside the grid: %g'):format(id, points[i]) end
      end
    end
  end
  local march = icons.parse(icons.PATHS.forcedMarch)
  if #march ~= 2 or table.concat(march[1], ' ') ~= '6 12 12 6 18 12' or table.concat(march[2], ' ') ~= '6 19 12 13 18 19' then
    return false, 'forcedMarch: the relative lines are wrong'
  end
  local bounty = icons.parse(icons.PATHS.bounty)
  local ok, why = onCircle(bounty[1], 12, 12, 7)
  if not ok then return false, 'bounty: ' .. why end
  local eye = icons.parse(icons.PATHS.eye)
  ok, why = onCircle(eye[2], 12, 12, 3)
  if not ok then return false, 'eye: ' .. why end
  -- The arch of kingKnight: a half circle of radius 6 from (6, 11) to (18, 11), with its top at y = 5.
  local arch = icons.parse(icons.PATHS.kingKnight)[1]
  local top = math.huge
  for i = 2, #arch, 2 do top = math.min(top, arch[i]) end
  if not near(top, 5, 0.05) then return false, ('kingKnight: the top of the arch is at %.2f, not 5'):format(top) end
  local star = icons.parse(icons.PATHS.earlyPromo)[1]
  if not (near(star[1], star[#star - 1]) and near(star[2], star[#star])) then return false, 'earlyPromo: the closed path does not end at its start' end
  -- The top of the coins: c0-1.7 3.1-3 7-3 s7 1.3 7 3. The S curve ends at (19, 8), and the top of the ellipse is at y = 5.
  local coins = icons.parse(icons.PATHS.coins)[1]
  local minY = math.huge
  for i = 2, #coins, 2 do minY = math.min(minY, coins[i]) end
  if not near(minY, 5, 0.05) then return false, ('coins: the top is at %.2f, not 5'):format(minY) end
  local dot = icons.parse(icons.PATHS.tag)[2]
  if not (near(dot[1], 7.5) and near(dot[3], 7.51)) then return false, 'tag: the dot is wrong' end
  return true
end

return {
  { 'expect', check, 'the path reader of icons.lua gives the shapes of the web icons' },
  { 'quit' },
}
