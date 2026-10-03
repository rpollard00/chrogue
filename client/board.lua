-- The board: the squares and their marks, the pieces, the gold of a capture, the promotion picker, and the banner of the floor.
local gfx = require('gfx')
local layout = require('layout')
local text = require('text')
local theme = require('theme')
local lg = love.graphics
local C, px = theme.color, gfx.px

local board = {}

local SQ = layout.square
-- A piece has 9.5% of the width of the board with no border, and a coordinate has 1.9%.
local INNER = layout.board.w - 2 * layout.boardBorder
local PIECE = 0.095 * INNER
local COORD = 0.019 * INNER
local FLOATER = 0.05 * INNER
local GRAY = theme.hex('#6f6f6f')

local function boardPoint(col, row)
  return layout.board.x + layout.boardBorder + col * SQ, layout.board.y + layout.boardBorder + row * SQ
end

local function squares(self)
  local check = self:checkSquare()
  local scouting = self:scouting()
  for s = 0, 63 do
    local f, r = s % 8, math.floor(s / 8)
    local x, y = layout.squareAt(s)
    gfx.rect(x, y, SQ, SQ, 0, (f + r) % 2 == 1 and C.light or C.dark)
    -- A square has one mark. The check is before the selection, and the selection is before the last move.
    local last = self.view.last
    if s == check then
      local mark = C.markCheck
      local t = self.alarmAt and self.time - self.alarmAt or self.ALARM_TIME
      if t < self.ALARM_TIME then mark = theme.mixNow(C.markAlarm, math.sin(math.pi * t / (self.ALARM_TIME / 2)) ^ 2, C.markCheck) end
      gfx.rect(x, y, SQ, SQ, 0, mark)
    elseif s == self.selected then
      gfx.rect(x, y, SQ, SQ, 0, scouting and C.scoutSelected or C.markSelected)
    elseif last and (s == last.from or s == last.to) then
      gfx.rect(x, y, SQ, SQ, 0, C.markLast)
    end
    if f == 0 then gfx.text(tostring(r + 1), 'semibold', COORD, x + SQ * 0.05, y + SQ * 0.03, { color = C.coord }) end
    if r == 0 then
      gfx.text(('abcdefgh'):sub(f + 1, f + 1), 'semibold', COORD, x, y + SQ * 0.98 - COORD * 1.2, { color = C.coord, align = 'right', width = SQ * 0.94 })
    end
    -- The marks of the moves. A move that captures has a ring, and a move to an empty square has a dot.
    local targets = self:targetsTo(s)
    if #targets > 0 then
      local capture = false
      for _, m in ipairs(targets) do capture = capture or m.capture == true end
      if capture then
        local width = math.max(px(3), 0.008 * INNER)
        gfx.ring(x + SQ / 2, y + SQ / 2, SQ * 0.46 - width / 2, width, scouting and C.scoutCapture or C.markCapture)
      else
        gfx.circle(x + SQ / 2, y + SQ / 2, SQ * 0.14, scouting and C.scoutTarget or C.markTarget)
      end
    end
  end
end

local function pieces(self)
  -- A captured piece becomes smaller and goes away below the piece that captured it.
  for _, sprite in ipairs(self.gone) do
    local t = math.min(1, (self.time - sprite.goneAt) / self.GONE_TIME)
    local x, y = boardPoint(sprite.col, sprite.row)
    gfx.piece(sprite.kind, sprite.color, x + SQ / 2, y + SQ / 2, PIECE, 1 - t * t, 1 - 0.6 * t * t)
  end
  local moving = {}
  local function draw(sprite)
    local e = gfx.ease((self.time - sprite.movedAt) / self.MOVE_TIME)
    local x, y = boardPoint(sprite.fromCol + (sprite.col - sprite.fromCol) * e, sprite.fromRow + (sprite.row - sprite.fromRow) * e)
    local scale = 1
    if sprite.promotedAt then
      -- A new piece of a promotion becomes larger for a moment, with a gold light.
      local t = (self.time - sprite.promotedAt - self.PROMOTE_DELAY) / self.PROMOTE_TIME
      if t > 0 and t < 1 then
        local k = t < 0.35 and t / 0.35 or 1 - (t - 0.35) / 0.65
        scale = 1 + 0.6 * k
        for i = 3, 1, -1 do gfx.circle(x + SQ / 2, y + SQ / 2, SQ * (0.3 + 0.12 * i) * scale, C.gold, 0.16 * k) end
      end
    end
    gfx.piece(sprite.kind, sprite.color, x + SQ / 2, y + SQ / 2, PIECE, 1, scale)
  end
  for _, sprite in pairs(self.sprites) do
    if self.time - sprite.movedAt < self.MOVE_TIME then moving[#moving + 1] = sprite else draw(sprite) end
  end
  for _, sprite in ipairs(moving) do draw(sprite) end
end

local function floaters(self)
  for _, floater in ipairs(self.floaters) do
    local t = (self.time - floater.at) / self.FLOAT_TIME
    local x, y = boardPoint(floater.col, floater.row)
    local alpha = t < 0.6 and 1 or 1 - (t - 0.6) / 0.4
    y = y - SQ * 0.9 * gfx.ease(t)
    local opts = { align = 'center', width = SQ, line = SQ, alpha = alpha, color = C.black }
    for _, d in ipairs({ { -1, 0 }, { 1, 0 }, { 0, -1 }, { 0, 1.6 } }) do
      gfx.text(floater.text, 'bold', FLOATER, x + d[1] * px(1.2), y + d[2] * px(1.2), opts)
    end
    opts.color = C.gold
    gfx.text(floater.text, 'bold', FLOATER, x, y, opts)
  end
end

-- The rectangles of the promotion picker. The picker is on the board. It starts at the square of the promotion.
function board.promotionRects(self)
  if not self.promotion then return nil end
  local x, y = layout.squareAt(self.promotion[1].to)
  local rects = {}
  for i = 1, #self.promotion do rects[i] = { x = x, y = y + (i - 1) * SQ, w = SQ, h = SQ } end
  return rects
end

local function promotion(self, ui)
  local rects = board.promotionRects(self)
  if not rects then return end
  local x, y, h = rects[1].x, rects[1].y, #rects * SQ
  gfx.shadow(x, y, SQ, h, 0, 0.6, 1.4, 0, 0.75)
  gfx.rect(x - px(3), y - px(3), SQ + px(6), h + px(6), 0, C.edge)
  gfx.gradientRect(x, y, SQ, h, 0, C.liningLo, C.lining)
  for i, r in ipairs(rects) do
    if ui.hover == 'promo' .. i then gfx.rect(r.x, r.y, r.w, r.h, 0, C.accent, 0.45) end
    gfx.piece(self.promotion[i].promo, 'w', r.x + SQ / 2, r.y + SQ / 2, PIECE)
  end
end

-- The banner at the start of a battle: the floor and the name of the enemy.
local function intro(self)
  if not self.introAt then return end
  local t = (self.time - self.introAt) / self.INTRO_TIME
  if t < 0 or t >= 1 then return end
  local alpha, scaleY = 1, 1
  if t < 0.12 then alpha, scaleY = t / 0.12, 0.4 + 0.6 * t / 0.12 elseif t > 0.78 then alpha = 1 - (t - 0.78) / 0.22 end
  local b, floor = layout.board, self.view.floor
  local boss = floor.boss
  local nameSize = boss and 2.2 or 1.8
  local h = 1 + 1.2 + nameSize * 1.2 + 1
  local flair = boss and C.danger or C.accent
  lg.push()
  lg.translate(0, b.y + b.h / 2)
  lg.scale(1, scaleY)
  gfx.withAlpha(alpha, function()
    gfx.rect(b.x, -h / 2, b.w, h, 0, theme.alpha(C.bg, 0.9))
    gfx.rect(b.x, -h / 2, b.w, px(2), 0, flair)
    gfx.rect(b.x, h / 2 - px(2), b.w, px(2), 0, flair)
    local kicker = (text.floor(floor) .. (boss and ' · Boss' or '')):upper()
    gfx.text(kicker, 'body', 0.8, b.x, -h / 2 + 1, { color = C.dim, tracking = 0.12, align = 'center', width = b.w, line = 1.2 })
    local name = boss and floor.name:upper() or floor.name
    gfx.text(name, 'display', nameSize, b.x, -h / 2 + 2.2,
      { color = boss and C.danger or C.text, tracking = boss and 0.12 or 0.04, align = 'center', width = b.w, line = nameSize * 1.2 })
  end)
  lg.pop()
end

function board.draw(self, ui)
  local b = layout.board
  gfx.rect(b.x, b.y, b.w, b.h, px(4), C.frame)
  squares(self)
  pieces(self)
  floaters(self)
  promotion(self, ui)
  if self.resultAt then
    -- The board becomes dark below the result. After a defeat, it also loses its color.
    local t = math.min(1, (self.time - self.resultAt) / 0.4)
    local lost = self.view.result.winner == 'b'
    if lost then gfx.rect(b.x, b.y, b.w, b.h, px(4), GRAY, 0.55 * t) end
    gfx.rect(b.x, b.y, b.w, b.h, px(4), C.black, (lost and 0.5 or 0.4) * t)
  end
  intro(self)
end

return board
