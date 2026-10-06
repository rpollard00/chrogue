--[[
  The auras of a battle: the badge of a piece that has a boon, and the zone of an aura in focus.
  A boon is what a piece gets from a near piece of its side (core/PROTOCOL.md, `auras`). The core tells which piece has
  which boon and where each zone is. This module draws these lists. It has no rule and it does not know a relic.
  battle.lua gives it each view and the pointer. board.lua draws the zones with the squares and the badges with the pieces.
]]
local gfx = require('gfx')
local layout = require('layout')
local theme = require('theme')
local lg = love.graphics
local C = theme.color

local aura = {}

local SQ = layout.square
-- Each size is a part of a square.
local BADGE = 0.24 * SQ
local RIM = 0.017 * SQ
local GAIN_TIME = 0.3
local RING_TIME = 0.4
local LOSS_TIME = 0.35
local ZONE_TIME = 0.12
local ZONE_INSET = 0.04 * SQ
local ZONE_LINE = 0.03 * SQ
local PULSE_TIME = 0.9
local PULSE = 0.35
local PULSE_END = 0.15

-- A solid shape from its outline, around (0, 0), with a size of 1. Each outline is visible from its center.
local function glyph(points)
  local vertices = { { 0, 0 } }
  for i = 1, #points, 2 do vertices[#vertices + 1] = { points[i], points[i + 1] } end
  vertices[#vertices + 1] = { points[1], points[2] }
  return lg.newMesh(vertices, 'fan', 'static')
end

local function star()
  local points = {}
  for i = 0, 7 do
    local r, a = i % 2 == 0 and 0.5 or 0.19, i * math.pi / 4
    points[#points + 1], points[#points + 2] = r * math.sin(a), -r * math.cos(a)
  end
  return points
end

--[[
  The boons that this module can show. Each boon has a set slot on the left side of the square: the center of its badge,
  as parts of a square. A piece with one boon keeps the slot of that boon. The slots are away from the coordinates of the
  board. A boon that is not in this table has no badge.
]]
local BOONS = {
  shield = { x = 0.2, y = 0.36, size = 0.14 * SQ, glyph = glyph({ -0.44, -0.5, 0.44, -0.5, 0.44, 0.02, 0.3, 0.3, 0, 0.5, -0.3, 0.3, -0.44, 0.02 }) },
  moves = { x = 0.2, y = 0.64, size = 0.16 * SQ, glyph = glyph(star()) },
}
local ORDER = { 'shield', 'moves' }

-- The color of a side: green for the player, and red for the enemy, as the medals of their fans.
local function flair(color) return color == 'b' and C.danger or C.relic end

local NONE = {}

function aura.new()
  -- badges[id]: the piece and its boons. leaving: the badges that go away. focus: the auras of the view in focus.
  -- shapes[color]: the squares of the zones in focus of one side. lit[color][relic]: the medals that the board points to.
  -- pulse[id]: the badge of a piece that refuses a capture of the selected piece: its boon, and the time when its
  -- pulse started. A pulse that ends also has the time of its end and the scale that it had then.
  return {
    badges = {}, leaving = {}, focus = {}, lit = { w = {}, b = {} }, pulse = {},
    shapes = { w = { squares = {}, alpha = 0 }, b = { squares = {}, alpha = 0 } },
  }
end

--[[
  Takes the boons of each piece from a view. `restAt` is nil for a view that starts no motion. Else it gives the time
  when a piece is at rest after the motion of the response: a new badge comes into view then, and a badge that the piece
  lost goes away now. A captured piece has no badge.
]]
function aura.sync(self, view, time, restAt)
  local before = self.badges
  self.badges = {}
  if not restAt then self.leaving = {} end
  for _, p in ipairs(view.pieces) do
    local old, now = before[p.id], nil
    -- A core from before the auras gives no list.
    for _, entry in ipairs(p.auras or NONE) do
      if BOONS[entry.boon] then
        now = now or { color = p.color, square = p.square, boons = {} }
        local kept = old and old.boons[entry.boon]
        local at = nil
        if kept then at = kept.at elseif restAt then at = restAt(p.id) end
        now.boons[entry.boon] = { relics = entry.relics, at = at }
      end
    end
    self.badges[p.id] = now
    for boon, lost in pairs(restAt and old and old.boons or NONE) do
      -- A badge that did not come into view goes away with no motion.
      if not (now and now.boons[boon]) and not (lost.at and lost.at > time) then
        self.leaving[#self.leaving + 1] = { id = p.id, square = p.square, boon = boon, color = p.color, at = time }
      end
    end
  end
end

-- Each badge comes into view again. The battle calls it one time, after the banner of the floor.
function aura.pop(self, time)
  for _, badge in pairs(self.badges) do
    for _, boon in pairs(badge.boons) do boon.at = time end
  end
end

-- The relics that give a boon to a piece now, for each color: relics[color][id] is true.
function aura.relics(self)
  local relics = { w = {}, b = {} }
  for _, badge in pairs(self.badges) do
    for _, boon in pairs(badge.boons) do
      for _, id in ipairs(boon.relics) do relics[badge.color][id] = true end
    end
  end
  return relics
end

local function has(list, value)
  for _, v in ipairs(list) do if v == value then return true end end
  return false
end

--[[
  Adds the auras of the view that a piece points to: `found[i]` is true for the aura at index `i` of `view.auras`.
  A piece points to an aura if it is a source of it, or if it has a badge from it: the boon and the relic of the badge are
  those of the aura, and the square of the piece is in its zone. A relic can have two auras of one boon, thus the index
  tells which aura it is.
]]
local function aurasOf(view, piece, found)
  for i, item in ipairs(view.auras or NONE) do
    if item.color == piece.color then
      local mine = has(item.sources, piece.id)
      for _, entry in ipairs(piece.auras or NONE) do
        mine = mine or (entry.boon == item.boon and has(entry.relics, item.relic) and has(item.zone, piece.square))
      end
      if mine then found[i] = true end
    end
  end
end

-- The scale of a badge that pulses: from 1 to 1 + PULSE and back. A pulse that ends goes from its last scale to 1.
local function pulseScale(pulse, time)
  if pulse.endAt then return 1 + (pulse.from - 1) * (1 - gfx.ease((time - pulse.endAt) / PULSE_END)) end
  return 1 + PULSE * (0.5 - 0.5 * math.cos(2 * math.pi * (time - pulse.at) / PULSE_TIME))
end

--[[
  Finds the auras in focus and the lit medals for this frame.
  `shown` has the id of the relic of the medal that shows its card, for each color, or nil. Each aura of that relic is in focus.
  `hover` is the piece under the pointer, or nil. Its auras are in focus, and the medals of their relics are lit.
  `selected` is the selected piece, or nil. The medals of the relics of its auras are lit.
  `denied` has the captures of the selected piece that a boon refuses (`denied` of the view). The badge of that boon on
  each target piece pulses, and the medals of its relics are lit. A denied capture puts no zone in focus.
  The zones in focus of one side are one shape. A shape comes into view and goes out of view in a short time.
]]
function aura.update(self, dt, time, view, shown, hover, selected, denied)
  local list = view.auras or NONE
  local focus, pointed = {}, {}
  for i, item in ipairs(list) do
    if shown[item.color] == item.relic then focus[i] = true end
  end
  if hover then aurasOf(view, hover, pointed) end
  for i in pairs(pointed) do focus[i] = true end
  if selected then aurasOf(view, selected, pointed) end
  self.lit = { w = {}, b = {} }
  for i in pairs(pointed) do self.lit[list[i].color][list[i].relic] = true end
  -- A pulse continues while its piece stays in the list, thus it starts at the size of the badge. A pulse that is
  -- not in the list goes back to that size in a short time.
  local before = self.pulse
  self.pulse = {}
  for _, item in ipairs(denied) do
    local badge = self.badges[item.target]
    local boon = badge and badge.boons[item.boon]
    if boon then
      local pulse = before[item.target]
      if not pulse or pulse.boon ~= item.boon or pulse.endAt then pulse = { boon = item.boon, at = time } end
      self.pulse[item.target] = pulse
      for _, id in ipairs(boon.relics) do self.lit[badge.color][id] = true end
    end
  end
  for id, pulse in pairs(before) do
    if not self.pulse[id] then
      if not pulse.endAt then pulse.from, pulse.endAt = pulseScale(pulse, time), time end
      if time - pulse.endAt < PULSE_END then self.pulse[id] = pulse end
    end
  end

  -- The auras in focus, in the order of the view, and the indexes of those of each side as text.
  self.focus = {}
  local parts = { w = '', b = '' }
  for i, item in ipairs(list) do
    if focus[i] then
      self.focus[#self.focus + 1] = item
      parts[item.color] = parts[item.color] .. i .. ' '
    end
  end
  local step = dt / ZONE_TIME
  for color, shape in pairs(self.shapes) do
    if parts[color] == '' then
      -- A shape that goes out of view keeps its squares.
      shape.view, shape.alpha = nil, math.max(0, shape.alpha - step)
    else
      -- The squares change only with the view and with the auras in focus.
      if shape.view ~= view or shape.parts ~= parts[color] then
        shape.view, shape.parts, shape.squares = view, parts[color], {}
        for _, item in ipairs(self.focus) do
          if item.color == color then
            for _, s in ipairs(item.zone) do shape.squares[s] = true end
          end
        end
      end
      shape.alpha = math.min(1, shape.alpha + step)
    end
  end
  for i = #self.leaving, 1, -1 do
    if time - self.leaving[i].at > LOSS_TIME then table.remove(self.leaving, i) end
  end
end

-- 'wait' before a new badge comes into view, 'gain' while it comes, and then 'rest'.
local function phase(boon, time)
  if not boon.at then return 'rest' end
  if time < boon.at then return 'wait' end
  return time - boon.at < RING_TIME and 'gain' or 'rest'
end

-- True if a piece has a badge.
function aura.any(self) return next(self.badges) ~= nil end

-- False while a badge comes into view or goes away. The test script uses it.
function aura.settled(self, time)
  if #self.leaving > 0 then return false end
  for _, badge in pairs(self.badges) do
    for _, boon in pairs(badge.boons) do
      if phase(boon, time) ~= 'rest' then return false end
    end
  end
  return true
end

-- Drawing

local function inZone(squares, f, r)
  return f >= 0 and f <= 7 and r >= 0 and r <= 7 and squares[r * 8 + f] == true
end

-- A corner where a shape turns around a square that is not in it: the lines of the two next squares meet here.
-- (cx, cy) is the corner of the square in the direction (df, dr).
local function corner(squares, f, r, df, dr, cx, cy, line, alpha)
  if inZone(squares, f + df, r) and inZone(squares, f, r + dr) and not inZone(squares, f + df, r + dr) then
    local i, w = ZONE_INSET, ZONE_LINE
    local px, py = df > 0 and cx - i - w or cx + i, dr > 0 and cy + i or cy - i - w
    gfx.rect(px, dr > 0 and cy or py, w, i + w, 0, line, alpha)
    gfx.rect(df > 0 and px or cx, py, i + w, w, 0, line, alpha)
  end
end

--[[
  Draws the part of the zones in focus on square `s`, with the top left corner of the square. The zones of one side
  are one shape: one tint on each square, and one line where the shape ends. The line is inside the shape. The shapes
  of the two sides are different shapes. Draw them before the mark of the square.
]]
function aura.drawZone(self, s, x, y)
  local f, r = s % 8, math.floor(s / 8)
  for color, shape in pairs(self.shapes) do
    local squares, alpha = shape.squares, shape.alpha
    if alpha > 0 and squares[s] then
      local own = color ~= 'b'
      gfx.rect(x, y, SQ, SQ, 0, own and C.zone or C.zoneFoe, alpha)
      local line = own and C.zoneLine or C.zoneFoeLine
      local i, w = ZONE_INSET, ZONE_LINE
      -- The squares above, at the right, below, and at the left. Rank 8 is at the top of the board.
      local up, right, down, left = inZone(squares, f, r + 1), inZone(squares, f + 1, r), inZone(squares, f, r - 1), inZone(squares, f - 1, r)
      -- A side of the square with no zone after it has a line. The line goes to the next square if the shape continues there.
      local x0, x1 = left and x or x + i, right and x + SQ or x + SQ - i
      local y0, y1 = up and y or y + i, down and y + SQ or y + SQ - i
      if not up then gfx.rect(x0, y + i, x1 - x0, w, 0, line, alpha) end
      if not down then gfx.rect(x0, y + SQ - i - w, x1 - x0, w, 0, line, alpha) end
      if not left then gfx.rect(x + i, y0, w, y1 - y0, 0, line, alpha) end
      if not right then gfx.rect(x + SQ - i - w, y0, w, y1 - y0, 0, line, alpha) end
      corner(squares, f, r, 1, 1, x + SQ, y, line, alpha)
      corner(squares, f, r, -1, 1, x, y, line, alpha)
      corner(squares, f, r, 1, -1, x + SQ, y + SQ, line, alpha)
      corner(squares, f, r, -1, -1, x, y + SQ, line, alpha)
    end
  end
end

-- A badge: a dark round plate with a rim and a solid picture in the color of the side.
local function plate(slot, color, x, y, scale, alpha)
  local cx, cy, r = x + slot.x * SQ, y + slot.y * SQ, BADGE / 2 * scale
  local side = flair(color)
  gfx.circle(cx, cy, r, C.badge, alpha)
  gfx.ring(cx, cy, r - RIM * scale / 2, RIM * scale, side, 0.7 * alpha)
  gfx.setColor(side, alpha)
  lg.draw(slot.glyph, cx, cy, 0, slot.size * scale)
end

--[[
  Draws the badges of a piece, with the top left corner of the place of its sprite. Draw them after the piece.
  A new badge becomes smaller to its size, with one ring. A badge that the piece lost becomes larger and goes away.
  The pulse of a badge that refuses a capture is not a motion of `aura.settled`.
  A badge keeps its size when its piece changes its size.
]]
function aura.drawBadges(self, id, x, y, time)
  local badge = self.badges[id]
  if not badge and #self.leaving == 0 then return end
  for _, name in ipairs(ORDER) do
    local boon, slot = badge and badge.boons[name], BOONS[name]
    if boon and phase(boon, time) ~= 'wait' then
      local t = boon.at and time - boon.at or RING_TIME
      if t < RING_TIME then
        local grow = gfx.ease(t / RING_TIME)
        gfx.ring(x + slot.x * SQ, y + slot.y * SQ, BADGE / 2 * (1 + 1.4 * grow), RIM, flair(badge.color), 1 - t / RING_TIME)
      end
      local scale = 1.8 - 0.8 * gfx.ease(t / GAIN_TIME)
      -- The badge that refuses a capture of the selected piece becomes larger and smaller.
      local pulse = self.pulse[id]
      if pulse and pulse.boon == name then scale = scale * pulseScale(pulse, time) end
      plate(slot, badge.color, x, y, scale, 1)
    end
  end
  for _, gone in ipairs(self.leaving) do
    if gone.id == id then
      local t = math.min(1, (time - gone.at) / LOSS_TIME)
      plate(BOONS[gone.boon], gone.color, x, y, 1 + 0.4 * gfx.ease(t), 1 - t)
    end
  end
end

-- The state of the auras, for the dump of the test script: the badges by square with the phase of each boon, the auras
-- in focus in the order of the view, the relics of the lit medals of each side, and the squares of the badges that pulse.
function aura.state(self, time)
  local badges, at = {}, {}
  local function boons(square, id)
    if not at[square] then
      at[square] = { square = square, id = id, boons = {} }
      badges[#badges + 1] = at[square]
    end
    return at[square].boons
  end
  for id, badge in pairs(self.badges) do
    for _, name in ipairs(ORDER) do
      local boon = badge.boons[name]
      if boon then
        local list = boons(badge.square, id)
        list[#list + 1] = { boon = name, phase = phase(boon, time), relics = boon.relics }
      end
    end
  end
  for _, gone in ipairs(self.leaving) do
    local list = boons(gone.square, gone.id)
    list[#list + 1] = { boon = gone.boon, phase = 'loss', relics = {} }
  end
  table.sort(badges, function(a, b) return a.square < b.square end)
  local focus = {}
  for i, item in ipairs(self.focus) do
    focus[i] = { relic = item.relic, color = item.color, boon = item.boon, zone = item.zone }
  end
  local function ids(set)
    local list = {}
    for id in pairs(set) do list[#list + 1] = id end
    table.sort(list)
    return list
  end
  local pulse = {}
  -- A pulse that ends is not in the list. A piece that the enemy captured in that time has no badge.
  for id, state in pairs(self.pulse) do
    if not state.endAt and self.badges[id] then pulse[#pulse + 1] = self.badges[id].square end
  end
  table.sort(pulse)
  return { badges = badges, focus = focus, lit = { player = ids(self.lit.w), enemy = ids(self.lit.b) }, pulse = pulse }
end

return aura
