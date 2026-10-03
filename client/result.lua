-- The element above the board: the result of the battle. The panel shows the rows of the view of the core.
local gfx = require('gfx')
local layout = require('layout')
local text = require('text')
local ui = require('ui')
local theme = require('theme')
local lg = love.graphics
local C, px = theme.color, gfx.px

local result = {}

local ROW_STEP = 0.26
local ROW = 1.5
local PAD = layout.result.pad

-- The rows of the gold tally, and the lines about the army: the number of the pieces that return, and the number of
-- the pieces that join. The core gives the rows in their order.
local function content(self)
  local res = self.view.result
  local rows = {}
  for _, row in ipairs(res.rows or {}) do rows[#rows + 1] = { label = text.resultRow(row), value = row.gold } end
  local title = text.OUTCOME[res.outcome] or res.outcome
  local army = {}
  if (res.rescued or 0) > 0 then army[#army + 1] = 'Pieces that return to your army: ' .. res.rescued end
  if #(res.recruits or {}) > 0 then army[#army + 1] = 'Pieces that join your army: ' .. #res.recruits end
  return { title = title, cause = text.cause(res), rows = rows, total = #rows > 1 and res.total or nil,
    army = army, winner = res.winner }
end

-- The positions of the parts of the result panel. The panel is above the layout, thus its height comes from its content.
function result.layout(self)
  if not self.view.result or not self.resultAt then return nil end
  -- The panel changes only with the view, thus the layout is kept for the next frames.
  local kept = self.resultCache
  if kept and kept.view == self.view and kept.u == gfx.u then return kept.c end
  local c = content(self)
  local w = layout.result.w
  local inner = w - 2 * PAD
  c.causeLines = gfx.wrap(c.cause, 'body', 1, inner, true)
  local y = PAD
  c.titleY = y
  y = y + 1.92 + 0.5
  c.causeY = y
  y = y + #c.causeLines * ROW + 0.75
  if #c.rows > 0 then
    c.rowsY = y
    y = y + #c.rows * ROW + (#c.rows - 1) * 0.25
    if c.total then
      c.totalY = y + 0.25 + 0.35
      y = c.totalY + 0.5 + 1.15 * 1.5
    end
    y = y + 1
  end
  if #c.army > 0 then
    c.armyY = y
    y = y + #c.army * ROW + 0.75
  end
  local keyW = gfx.textWidth('CONTINUE', 'display', 1.3, 0.05) + 3
  local h = y + 3 + px(4) + PAD
  local b = layout.board
  c.x, c.y, c.w, c.h = b.x + (b.w - w) / 2, b.y + (b.h - h) / 2, w, h
  c.key = { x = c.x + (w - keyW) / 2, y = c.y + y, w = keyW, h = 3 }
  self.resultCache = { view = self.view, u = gfx.u, c = c }
  return c
end

function result.draw(self, pointer)
  local c = result.layout(self)
  if not c then return end
  local since = self.time - self.resultAt
  local t = math.min(1, since / 0.32)
  local scale = 0.85 + 0.15 * gfx.ease(t) + 0.03 * math.sin(math.pi * t)
  local border = c.winner == 'w' and C.accent or (c.winner == 'b' and C.danger or C.line)
  lg.push()
  lg.translate(c.x + c.w / 2, c.y + c.h / 2)
  lg.scale(scale)
  lg.translate(-(c.x + c.w / 2), -(c.y + c.h / 2))
  gfx.withAlpha(math.min(1, t * 1.5), function()
    gfx.shadow(c.x, c.y, c.w, c.h, px(10), px(10), px(40), 0, 0.6)
    gfx.rect(c.x, c.y, c.w, c.h, px(10), C.panel)
    gfx.outline(c.x, c.y, c.w, c.h, px(10), px(1), border)
    local x, inner = c.x + PAD, c.w - 2 * PAD
    local titleColor = c.winner == 'w' and C.accent or (c.winner == 'b' and C.danger or C.text)
    local title = { color = titleColor, tracking = 0.04, align = 'center', width = inner, line = 1.92 }
    if c.winner == 'w' then
      -- The heading of a win has a light for a moment.
      local glow = math.max(0, 1 - math.abs(since / 1.6 - 0.3) / 0.3)
      for i = 1, 3 do
        gfx.rect(x + inner / 2 - 4 - i * 0.5, c.y + c.titleY + 0.4 - i * 0.2, 8 + i, 1.1 + i * 0.4, 0.8, C.accent, 0.07 * glow)
      end
    end
    gfx.text(c.title:upper(), 'display', 1.6, x, c.y + c.titleY, title)
    gfx.lines(c.causeLines, 'body', 1, x, c.y + c.causeY, ROW, { align = 'center', width = inner })
    for i, row in ipairs(c.rows) do
      ui.tallyRow(x, c.y + c.rowsY + (i - 1) * (ROW + 0.25), inner, row.label, row.value, since - (i - 1) * ROW_STEP)
    end
    if c.total then
      gfx.rect(x, c.y + c.totalY, inner, px(1), 0, C.line)
      ui.tallyRow(x, c.y + c.totalY + 0.5, inner, 'Total gold', c.total, since - #c.rows * ROW_STEP, true)
    end
    if c.armyY then gfx.lines(c.army, 'body', 1, x, c.y + c.armyY, ROW, { color = C.dim, align = 'center', width = inner }) end
    local k = c.key
    ui.key(k, 'Continue', 'primary', ui.state(pointer, 'continue'))
    if c.winner == 'w' then ui.burst(c.x + c.w / 2, c.y + c.titleY + 0.96, since, 16, 0.15) end
  end)
  lg.pop()
end

return result
