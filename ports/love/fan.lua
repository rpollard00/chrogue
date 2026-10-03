-- The fan: the relics of the player, or the traits of the enemy, as medals that overlap with a fixed step.
-- The medal under the pointer shows the card of the relic. The card is above the layout.
local gfx = require('gfx')
local icons = require('icons')
local layout = require('layout')
local theme = require('theme')
local lg = love.graphics
local C, px = theme.color, gfx.px

local fan = {}

local RAISE = 0.35
local RAISE_TIME = 0.1
local TIP_TIME = 0.12
local FLASH_TIME = 1.1

-- `items` are the relics or the traits of a view: id, name, text. The text of a trait is the text for the enemy.
function fan.new(items, side, area, align)
  local ids = {}
  for i, item in ipairs(items) do ids[i] = item.id end
  local self = { items = items, ids = ids, side = side, area = area, hovered = nil, hoveredAt = 0, raise = {} }
  for i = 1, #ids do self.raise[i] = 0 end
  self.flair = side == 'enemy' and C.danger or C.relic
  -- In the battle, the medals of the enemy are at the right side of their area, next to the stash.
  align = align or (side == 'enemy' and 'right' or 'left')
  self.left = align == 'right' and area.x + area.w - #ids * layout.fanStep or area.x
  return self
end

-- The center of a medal when it is not raised.
function fan.center(self, i)
  return self.left + (i - 1) * layout.fanStep + layout.chip / 2, self.area.y + layout.chip / 2
end

local function inside(self, i, x, y)
  local cx, cy = fan.center(self, i)
  return (x - cx) ^ 2 + (y - cy) ^ 2 <= (layout.chip / 2) ^ 2
end

-- Finds the medal under the pointer. A medal is above the medals before it. The medal that shows its card is above all.
function fan.update(self, x, y, time, dt)
  local found = nil
  if x then
    if self.hovered and inside(self, self.hovered, x, y) then
      found = self.hovered
    else
      for i = #self.ids, 1, -1 do
        if inside(self, i, x, y) then found = i break end
      end
    end
  end
  if found ~= self.hovered then self.hovered, self.hoveredAt = found, time end
  for i = 1, #self.ids do
    local target = i == found and 1 or 0
    local step = dt / RAISE_TIME
    self.raise[i] = self.raise[i] + math.max(-step, math.min(step, target - self.raise[i]))
  end
end

-- The area that the medals can draw on: the fan with the space for a raised medal and for the ring of a flash.
function fan.bounds(self)
  local a = self.area
  return { x = a.x - 0.5, y = a.y - 0.9, w = a.w + 1.6, h = a.h + 1.8 }
end

local function medalWithIcon(id, cx, cy, size, flair)
  gfx.medal(cx, cy, size, flair)
  gfx.setColor(flair)
  icons.draw(id, cx, cy, size * 0.56)
end

local function chip(self, i, flashes, time)
  local id = self.ids[i]
  local cx, cy = fan.center(self, i)
  local direction = self.side == 'enemy' and 1 or -1
  cy = cy + direction * RAISE * self.raise[i]
  local r = layout.chip / 2
  local flashed = flashes and flashes[id]
  local flash = flashed and 1 - gfx.ease((time - flashed) / FLASH_TIME) or 0
  if self.hovered == i then
    gfx.shadow(cx - r, cy - r, 2 * r, 2 * r, r, px(6), px(10), 0, 0.7)
    gfx.circle(cx, cy, r + px(1.5), self.flair)
  else
    gfx.circle(cx - px(2), cy, r + px(1), C.black, 0.3)
    gfx.circle(cx, cy + px(2), r, theme.hex('#0d0e12'))
  end
  if flash > 0 then gfx.circle(cx, cy, r + 0.3 * flash, C.flash, flash) end
  gfx.gradient(function() lg.circle('fill', cx, cy, r, 48) end, cx - r, cy - r, 2 * r, 2 * r, C.chipHi, C.chipLo)
  medalWithIcon(id, cx, cy, 1.9, self.flair)
  if flash > 0 then
    lg.setBlendMode('add')
    gfx.circle(cx, cy, r, C.white, 0.3 * flash)
    lg.setBlendMode('alpha')
  end
end

-- Draws the medals. `flashes` has the time of the last effect of each relic.
function fan.draw(self, flashes, time)
  for i = 1, #self.ids do
    if i ~= self.hovered then chip(self, i, flashes, time) end
  end
  if self.hovered then chip(self, self.hovered, flashes, time) end
end

-- The rectangle of the card of the medal under the pointer, or nil. The card of the player is above the fan, and the card
-- of the enemy is below it. Near a side of the stage, the card moves, thus the full card stays in view.
function fan.cardRect(self)
  if not self.hovered then return nil end
  local card = layout.card
  local center = self.left + (self.hovered - 1) * layout.fanStep + layout.fanStep / 2
  center = math.max(layout.edge + card.w / 2, math.min(center, layout.stage.w - layout.edge - card.w / 2))
  local y = self.side == 'enemy' and self.area.y + self.area.h + card.gap or self.area.y - card.gap - card.h
  return { x = center - card.w / 2, y = y, w = card.w, h = card.h }
end

-- The opacity and the scale of the card while it comes into view.
function fan.cardEnter(self, time)
  local t = math.min(1, (time - self.hoveredAt) / TIP_TIME)
  return t, 0.96 + 0.04 * t
end

-- Draws the card of a relic in a rectangle. It has the size of each card of the game, and its text is in the place of the key.
function fan.drawCard(self, r)
  local id = self.ids[self.hovered]
  local relic = self.items[self.hovered]
  local enemy = self.side == 'enemy'
  local text = relic.text
  local flair = self.flair
  local x, y, w = r.x, r.y, r.w
  gfx.shadow(x, y, w, r.h, px(10), px(18), px(24), -px(6), 0.9)
  gfx.raised(x, y, w, r.h, px(10), { tint = flair, rim = theme.mix(flair, 0.7, theme.hex('#111111')) })
  gfx.outline(x + 0.3, y + 0.3, w - 0.6, r.h - 0.6, px(7), px(1), flair, 0.35)
  gfx.text(enemy and 'ENEMY TRAIT' or 'RELIC', 'bold', 0.68, x + 0.85, y + 0.65, { color = flair, tracking = 0.12, line = 1.25 })
  medalWithIcon(id, x + w / 2, y + 2.45 + 1.5, 3, flair)
  gfx.text(relic.name, 'bold', 0.95, x, y + 5.85, { align = 'center', width = w, line = 1.1 })
  local lines = gfx.wrap(text, 'body', 0.75, w - 1.7, true)
  gfx.lines(lines, 'body', 0.75, x, y + 7.35, 0.75 * 1.35, { align = 'center', width = w })
end

return fan
