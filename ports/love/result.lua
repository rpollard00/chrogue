-- The elements above the board: the result of the battle, and the question before the player gives up.
local gfx = require('gfx')
local layout = require('layout')
local screen = require('screen')
local theme = require('theme')
local lg = love.graphics
local C, px = theme.color, gfx.px

local result = {}

local ROW_STEP = 0.26
local ROW = 1.5
local PAD = layout.result.pad

-- The rows of the gold tally, and the number of the pieces that return.
local function content(self)
  local res = self.battle.result
  local reward, rows, rescued = res.reward, {}, 0
  if res.winner ~= 'b' then
    rows[#rows + 1] = { label = 'Gold from captures', value = reward.captures }
    if reward.clear ~= 0 then rows[#rows + 1] = { label = 'Gold for the win', value = reward.clear } end
    for _, bonus in ipairs(reward.bonuses) do rows[#rows + 1] = { label = 'Gold from ' .. bonus.label, value = bonus.gold } end
    rescued = #self.battle.rescued
  end
  local total = 0
  for _, row in ipairs(rows) do total = total + row.value end
  local title = res.winner == 'w' and 'Victory' or (res.winner == 'b' and 'Defeat' or 'Draw')
  local cause = res.winner == nil and screen.DRAW_TEXT[res.reason] or screen.WIN_TEXT[res.winner][res.reason]
  return { title = title, cause = cause, rows = rows, total = #rows > 1 and total or nil, rescued = rescued, winner = res.winner }
end

-- The positions of the parts of the result panel. The panel is above the layout, thus its height comes from its content.
function result.layout(self)
  if not self.battle.result then return nil end
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
  if c.rescued > 0 then
    c.rescuedY = y
    y = y + ROW + 0.75
  end
  local keyW = gfx.textWidth('CONTINUE', 'display', 1.3, 0.05) + 3
  local h = y + 3 + px(4) + PAD
  local b = layout.board
  c.x, c.y, c.w, c.h = b.x + (b.w - w) / 2, b.y + (b.h - h) / 2, w, h
  c.key = { x = c.x + (w - keyW) / 2, y = c.y + y, w = keyW, h = 3 }
  return c
end

-- A number that counts from 0 to its value.
local function counted(value, t)
  t = math.max(0, math.min(1, t / screen.COUNT_TIME))
  return tostring(math.floor(value * (1 - (1 - t) ^ 3) + 0.5))
end

local function tallyRow(c, label, value, y, since, total)
  local t = since / 0.3
  if t <= 0 then return end
  local x, inner = c.x + PAD, c.w - 2 * PAD
  local size, face = total and 1.15 or 1, total and 'semibold' or 'body'
  gfx.withAlpha(math.min(1, t), function()
    local rise = 0.4 * (1 - gfx.ease(t))
    local line = size * 1.5
    gfx.text(label, face, size, x, y + rise, { color = total and C.text or C.dim, line = line })
    gfx.text(counted(value, since), 'semibold', size, x, y + rise, { color = C.gold, align = 'right', width = inner, line = line })
  end)
end

-- Sparks that fly out from the heading after a win.
local function burst(cx, cy, since)
  local t = (since - 0.15) / 1.1
  if t <= 0 or t >= 1 then return end
  local e = gfx.ease(t)
  for i = 0, 15 do
    local angle = i / 16 * 2 * math.pi
    local reach = (3 + (i % 3) * 1.25) * e
    local small = i % 2 == 0
    local r = (small and 0.15 or 0.25) * (1 - 0.7 * e)
    gfx.circle(cx + math.sin(angle) * reach, cy - math.cos(angle) * reach, r, small and C.light or C.accent, 1 - e)
  end
end

function result.draw(self, ui)
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
      tallyRow(c, row.label, row.value, c.y + c.rowsY + (i - 1) * (ROW + 0.25), since - (i - 1) * ROW_STEP)
    end
    if c.total then
      gfx.rect(x, c.y + c.totalY, inner, px(1), 0, C.line)
      tallyRow(c, 'Total gold', c.total, c.y + c.totalY + 0.5, since - #c.rows * ROW_STEP, true)
    end
    if c.rescuedY then
      gfx.text('Pieces that return to your army: ' .. c.rescued, 'body', 1, x, c.y + c.rescuedY, { color = C.dim, align = 'center', width = inner, line = ROW })
    end
    local k = c.key
    local down = gfx.key(k.x, k.y, k.w, k.h, 'primary', { hover = ui.hover == 'continue', pressed = ui.pressed == 'continue' })
    gfx.text('CONTINUE', 'display', 1.3, k.x, k.y + down, { color = C.amberInk, tracking = 0.05, align = 'center', width = k.w, line = k.h })
    if c.winner == 'w' then burst(c.x + c.w / 2, c.y + c.titleY + 0.96, since) end
  end)
  lg.pop()
end

-- The question before the player gives up. The web game uses the dialog of the browser for this question.
local QUESTION = 'The run will end. Give up?'

function result.confirmLayout()
  local w, h = 20, 7.4
  local x, y = (layout.stage.w - w) / 2, (layout.stage.h - h) / 2
  return {
    x = x, y = y, w = w, h = h,
    cancel = { x = x + w - 1.25 - 4.2 - 0.7 - 5.2, y = y + h - 1.25 - 2.25, w = 5.2, h = 2.25 },
    ok = { x = x + w - 1.25 - 4.2, y = y + h - 1.25 - 2.25, w = 4.2, h = 2.25 },
  }
end

function result.drawConfirm(self, ui)
  if not self.confirm then return end
  local d = result.confirmLayout()
  gfx.rect(0, 0, layout.stage.w, layout.stage.h, 0, C.black, 0.45)
  gfx.shadow(d.x, d.y, d.w, d.h, px(10), px(10), px(40), 0, 0.6)
  gfx.rect(d.x, d.y, d.w, d.h, px(10), C.panel)
  gfx.outline(d.x, d.y, d.w, d.h, px(10), px(1), C.line)
  gfx.text(QUESTION, 'body', 1.05, d.x + 1.25, d.y + 1.25, { line = 1.6 })
  for _, key in ipairs({ { 'cancel', 'Cancel' }, { 'ok', 'OK' } }) do
    local k = d[key[1]]
    local down = gfx.key(k.x, k.y, k.w, k.h, 'key', { hover = ui.hover == key[1], pressed = ui.pressed == key[1] })
    gfx.text(key[2], 'semibold', 0.9, k.x, k.y + down, { align = 'center', width = k.w, line = k.h })
  end
end

return result
