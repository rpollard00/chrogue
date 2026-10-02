-- The two plaques: the enemy above the board, and the player below it. Each plaque is a raised bar with set slots.
local gfx = require('gfx')
local icons = require('icons')
local layout = require('layout')
local screen = require('screen')
local theme = require('theme')
local lg = love.graphics
local C, px = theme.color, gfx.px

local plaques = {}

local ORDER = { 'k', 'q', 'r', 'b', 'n', 'p' }
local PIECE_SIZE = 1.6

local function kingMedal(r, color, flair)
  local cx, cy = r.x + r.w / 2, r.y + r.h / 2
  gfx.medal(cx, cy, r.w, flair, true)
  gfx.piece('k', color, cx, cy, r.w * 0.56 * 1.2)
end

-- The stash: the last piece that a side captured, and the number of the pieces.
local function stash(r, types, color, changedAt, time)
  gfx.well(r.x, r.y, r.w, r.h, px(9), true)
  local count = #types
  if count == 0 then return end
  local scale = 1
  if changedAt then
    -- The new piece comes into view larger than its size, then it goes to its size.
    local t = (time - changedAt) / screen.PIP_TIME
    if t < 0.6 then scale = 1.7 * t / 0.6 elseif t < 1 then scale = 1.7 - 0.7 * (t - 0.6) / 0.4 end
  end
  local half = gfx.textWidth('♜', 'piece', PIECE_SIZE) / 2
  if scale > 0.05 then gfx.piece(types[count], color, r.x + 0.6 + half, r.y + r.h / 2, PIECE_SIZE, 1, scale) end
  gfx.text('×' .. count, 'display', 1, r.x, r.y, { color = C.liningInk, align = 'right', width = r.w - 0.6, line = r.h })
end

-- The list of all the pieces of a stash. It is above the layout, and it shows while the pointer is on the stash.
function plaques.stashList(self, side)
  local enemy = side == 'enemy'
  local r = enemy and layout.foe.stash or layout.me.stash
  local types = enemy and self.battle.taken.b or self.battle.taken.w
  if #types == 0 then return end
  local color = enemy and 'w' or 'b'
  local sorted = {}
  for _, kind in ipairs(ORDER) do
    for _, t in ipairs(types) do if t == kind then sorted[#sorted + 1] = t end end
  end
  local step = gfx.textWidth('♜', 'piece', PIECE_SIZE) + 0.1
  local perRow = math.max(1, math.floor(17.5 / step))
  local rows = math.ceil(#sorted / perRow)
  local caption = enemy and 'LOST' or 'CAPTURED'
  local w = math.max(math.min(#sorted, perRow) * step - 0.1, gfx.textWidth(caption, 'bold', 0.66, 0.1)) + 1.5
  local h = 0.5 + 0.99 + 0.3 + rows * 1.8 + 0.6
  local x = enemy and r.x + r.w - w or r.x
  local y = enemy and r.y + r.h + 0.9 or r.y - 0.9 - h
  gfx.shadow(x, y, w, h, px(9), px(14), px(20), -px(6), 0.75)
  gfx.rect(x, y + px(3), w, h, px(9), C.edge)
  gfx.gradientRect(x, y, w, h, px(9), C.liningHi, C.lining)
  gfx.text(caption, 'bold', 0.66, x + 0.75, y + 0.5, { color = C.liningInk, tracking = 0.1, line = 0.99 })
  for i, kind in ipairs(sorted) do
    local col, row = (i - 1) % perRow, math.floor((i - 1) / perRow)
    gfx.piece(kind, color, x + 0.75 + col * step + (step - 0.1) / 2, y + 0.5 + 0.99 + 0.3 + row * 1.8 + 0.9, PIECE_SIZE)
  end
end

local function name(r, text)
  gfx.text(text:upper(), 'display', 1.45, r.x, r.y, { tracking = 0.035, line = r.h, shadow = C.shadeSoft })
end

function plaques.enemy(self, foeFan, drawFan)
  local L = layout.foe
  local p = L.plaque
  gfx.raised(p.x, p.y, p.w, p.h, px(12))
  kingMedal(L.medal, 'b', C.danger)
  name(L.name, self.spec.name)
  local sub = ('Floor %d of %d'):format(self.run.floor, self.floors)
  local width = gfx.text(sub, 'body', 0.8, L.sub.x, L.sub.y, { color = C.dim, line = L.sub.h })
  if self.spec.boss then
    local bx, by, bh = L.sub.x + width + 0.45, L.sub.y + 0.08, 1.04
    local bw = gfx.textWidth('BOSS', 'display', 0.72, 0.12) + 0.8 - 0.12 * 0.72
    gfx.rect(bx, by + px(1), bw, bh, px(3), C.bossEdge)
    gfx.gradientRect(bx, by, bw, bh, px(3), C.bossHi, C.danger)
    gfx.text('BOSS', 'display', 0.72, bx + 0.4, by, { color = C.bossInk, tracking = 0.12, line = bh })
  end
  drawFan(foeFan)
  stash(L.stash, self.battle.taken.b, 'w', self.stashAt.b, self.time)
end

-- The lamp: the turn status. It is lit when the player can move.
local function lamp(self, r)
  local text, lit = screen.status(self)
  local cx, cy = r.x + 0.3, r.y + r.h / 2
  gfx.circle(cx, cy, 0.3 + px(1.5), C.keyEdge)
  if lit then
    for i = 4, 1, -1 do gfx.circle(cx, cy, 0.3 + px(2) * i, C.accent, 0.09) end
    gfx.radial(cx, cy, 0.3, C.lampCore, C.accent)
  else
    gfx.circle(cx, cy, 0.3, C.wellHi)
  end
  local color = lit and C.lamp or C.dim
  local width = gfx.text(text, 'bold', 1.05, r.x + 1.05, r.y, { color = color, line = r.h })
  if self.busy then
    -- Three dots that blink one after the other.
    for i = 0, 2 do
      local phase = (self.time - i * 0.15) % 1
      local glow = phase < 0.3 and phase / 0.3 or (phase < 0.6 and 1 - (phase - 0.3) / 0.3 or 0)
      local dot = gfx.textWidth('.', 'bold', 1.05)
      gfx.text('.', 'bold', 1.05, r.x + 1.05 + width + i * dot, r.y, { color = color, alpha = 0.25 + 0.75 * glow, line = r.h })
    end
  end
end

-- The purse: the gold of the run, and the gold from the captures of this battle.
local function purse(self, r)
  gfx.well(r.x, r.y, r.w, r.h, px(9))
  local x = r.x + 0.75
  gfx.setColor(C.gold)
  icons.coin(x + 0.675, r.y + r.h / 2, 1.35)
  x = x + 1.35 + 0.405
  x = x + gfx.text(tostring(self.run.gold), 'display', 1.35, x, r.y, { color = C.gold, tracking = 0.02, line = r.h }) + 0.5
  if self.gold.to > 0 then
    local opts = { color = C.dim, line = r.h }
    x = x + gfx.text('+', 'body', 0.8, x, r.y, opts)
    x = x + gfx.text(tostring(screen.shownGold(self)), 'bold', 0.8, x, r.y, { color = C.gold, line = r.h })
    gfx.text(' from captures', 'body', 0.8, x, r.y, opts)
  end
end

function plaques.player(self, myFan, drawFan, ui)
  local L = layout.me
  local p = L.plaque
  gfx.raised(p.x, p.y, p.w, p.h, px(12), { top = C.accent })
  kingMedal(L.medal, 'w', C.medalRim)
  name(L.name, 'You')
  local over = self.battle.result ~= nil
  -- The lamp and the Give up key go away when the battle has a result. Their slots stay.
  if not over then lamp(self, L.lamp) end
  purse(self, L.purse)
  stash(L.stash, self.battle.taken.w, 'b', self.stashAt.w, self.time)
  drawFan(myFan)
  if not over then
    local k = L.giveUp
    local hover, pressed = ui.hover == 'giveUp', ui.pressed == 'giveUp'
    local down = gfx.key(k.x, k.y, k.w, k.h, 'quiet', { hover = hover, pressed = pressed })
    local color = hover and theme.mix(C.danger, 0.6, C.text) or C.dim
    gfx.text('Give up', 'body', 0.82, k.x, k.y + down, { color = color, align = 'center', width = k.w, line = k.h })
  end
end

return plaques
