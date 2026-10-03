-- The end of a run: the port of viewOver in src/ui/views/over.ts. The king that fell, the heading, the tally of the
-- crowns, and the menu. The positions are set (layout.over); a lost run has empty space where a won run has more rows.
local gfx = require('gfx')
local layout = require('layout')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local lg = love.graphics
local C, px = theme.color, gfx.px

local over = {}
over.__index = over

local L = layout.over
-- The tally starts after the king falls.
local TALLY_START = 0.9

function over.new(app, view)
  return setmetatable({ app = app, view = view, time = 0 }, over)
end

function over:apply(view) self.view = view end
function over:refused(view) self.view = view end
function over:update(dt) self.time = self.time + dt end
function over:settled() return self.time > TALLY_START + 1.5 end

local KEYS = {
  { 'newRun', 'New run', 'primary', 'primary' },
  { 'upgrades', 'Upgrades', 'left', 'key' },
  { 'title', 'Title', 'right', 'key' },
}

function over:hit(x, y)
  for _, k in ipairs(KEYS) do
    if layout.contains(L.menu[k[3]], x, y) then return k[1] end
  end
end

function over:control(name)
  for _, k in ipairs(KEYS) do
    if k[1] == name then return L.menu[k[3]] end
  end
end

function over:activate(name)
  local app = self.app
  if app.waiting() then return end
  if name == 'newRun' then app.send({ cmd = 'new_run' })
  elseif name == 'upgrades' then app.send({ cmd = 'open_upgrades' })
  elseif name == 'title' then app.send({ cmd = 'to_title' }) end
end

function over:key(key)
  if key == 'return' or key == 'kpenter' then
    self:activate('newRun')
    return true
  end
  return false
end

-- The king that fell: the Black King after a win, your king after a defeat. It topples, then sparks fly after a win.
local function emblem(r, won, since)
  local cx, cy = r.x + r.w / 2, r.y + r.h / 2
  local t = math.min(1, since / 0.9)
  -- cubic-bezier(0.5, 0, 0.6, 1.4): the king falls past its end angle a little, then rests.
  local e = t < 1 and (t * t * (3 - 2 * t)) * (1 + 0.12 * math.sin(math.pi * t)) or 1
  lg.push()
  lg.translate(cx, cy)
  lg.rotate(math.rad(78) * e)
  if won then
    -- The fallen Black King has the dim color and no outline.
    local font = gfx.font('piece', 4.5)
    local s = 1 / gfx.u
    gfx.setColor(C.dim)
    lg.print('♚', font, -font:getWidth('♚') * s / 2, -(font:getAscent() - font:getDescent()) * s / 2, 0, s, s)
  else
    gfx.piece('k', 'w', 0, 0, 4.5)
  end
  lg.pop()
  if won then ui.burst(cx, cy, since, 20, 0.85) end
end

function over:draw(pointer)
  local v = self.view
  local s = v.summary
  local since = self.time
  emblem(L.emblem, s.won, since)
  local heading = s.won and 'The Black King falls' or 'Your king fell'
  local h = L.heading
  local color = s.won and C.accent or C.text
  if s.won then
    local glow = math.max(0, 1 - math.abs(since / 1.6 - 0.3) / 0.3)
    for i = 1, 3 do gfx.rect(h.x + h.w / 2 - 12 - i, h.y + 0.6 - i * 0.2, 24 + 2 * i, 1.8 + i * 0.4, 0.9, C.accent, 0.06 * glow) end
  end
  gfx.text(heading:upper(), 'display', 2.5, h.x, h.y, { color = color, tracking = 0.04, align = 'center', width = h.w, line = h.h })
  -- The line, and the badge of a new best.
  local line = ('You cleared %d of %d floors.'):format(s.cleared, s.floors)
  local l = L.line
  local lw = gfx.textWidth(line, 'body', 1)
  local badge, bw = 'NEW BEST', 0
  if s.new_best then bw = gfx.textWidth(badge, 'semibold', 0.8, 0.08) + 1 + 0.3 end
  local x = l.x + (l.w - lw - bw) / 2
  gfx.text(line, 'body', 1, x, l.y, { line = l.h })
  if s.new_best then
    local t = (since - 0.9) / 0.5
    local scale = t <= 0 and 0 or (t < 0.6 and 1.7 * t / 0.6 or (t < 1 and 1.7 - 0.7 * (t - 0.6) / 0.4 or 1))
    local bx, by, bh = x + lw + 0.3, l.y + (l.h - 1.32) / 2, 1.32
    local w = bw - 0.3
    if scale > 0.01 then
      gfx.scaled(bx + w / 2, by + bh / 2, scale, scale, function()
        gfx.outline(bx, by, w, bh, bh / 2, px(1), C.accent)
        gfx.text(badge, 'semibold', 0.8, bx, by, { color = C.accent, tracking = 0.08, align = 'center', width = w, line = bh })
      end)
    end
  end
  -- The tally of crowns, in a recessed slot.
  local t = L.tally
  gfx.well(t.x, t.y, t.w, t.h, px(9))
  local rows = v.rows or {}
  local r = L.rows
  for i, row in ipairs(rows) do
    if r.y[i] then ui.tallyRow(r.x, r.y[i], r.w, text.overRow(row), row.crowns, since - TALLY_START - (i - 1) * ui.ROW_STEP) end
  end
  if #rows > 1 then
    -- The core gives the total. The client does not add the rows.
    local total = s.crowns
    local start = since - TALLY_START - #rows * ui.ROW_STEP
    if start > 0 then gfx.rect(r.x, r.total, r.w, px(1), 0, C.line, math.min(1, start / 0.3)) end
    ui.tallyRow(r.x, r.total + 0.5, r.w, 'Total crowns', total, start, true)
  end
  local m = L.menu
  gfx.raised(m.bar.x, m.bar.y, m.bar.w, m.bar.h, px(12))
  for _, k in ipairs(KEYS) do ui.key(m[k[3]], k[2], k[4], ui.state(pointer, k[1])) end
end

function over:state() return { heading = self.view.summary.won and 'The Black King falls' or 'Your king fell' } end

return over
