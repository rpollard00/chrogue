-- The fan: the relics of the player, or the traits of the enemy, as medals that overlap with a fixed step.
-- The medal under the pointer shows the card of the relic. The card is above the layout.
-- The fan of the player also shows the relic slots of the run: a slot with no relic is an empty recessed ring at its
-- place in the fan. In the camp, the player can select a medal: it has an amber ring, and its card stays in view.
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
local SLIDE_TIME = 0.25
-- The radius of an empty slot. The slots do not overlap: their step is larger than their width.
local SLOT = 0.8

-- `items` are the relics or the traits of a view: id, name, text. The text of a trait is the text for the enemy.
-- `slots` is the number of the relic slots of the run, or nil for a fan with no slots (the traits of the enemy).
function fan.new(items, side, area, align, slots)
  local ids = {}
  for i, item in ipairs(items) do ids[i] = item.id end
  local self = { items = items, ids = ids, side = side, area = area, hovered = nil, selected = nil, shownAt = 0, raise = {},
    slots = slots or 0, slide = {}, slideAt = nil, time = 0 }
  for i = 1, #ids do self.raise[i], self.slide[i] = 0, 0 end
  self.flair = side == 'enemy' and C.danger or C.relic
  -- In the battle, the medals of the enemy are at the right side of their area, next to the stash.
  align = align or (side == 'enemy' and 'right' or 'left')
  self.left = align == 'right' and area.x + area.w - #ids * layout.fanStep or area.x
  return self
end

-- The center of a slot of the fan: the place of medal `i`.
function fan.slot(self, i)
  return self.left + (i - 1) * layout.fanStep + layout.chip / 2, self.area.y + layout.chip / 2
end

-- The center of a medal when it is not raised. A medal that goes to a new slot is on its way for a moment.
function fan.center(self, i)
  local cx, cy = fan.slot(self, i)
  local from = self.slide[i] or 0
  if from ~= 0 and self.slideAt then
    cx = cx + from * layout.fanStep * (1 - gfx.ease((self.time - self.slideAt) / SLIDE_TIME))
  end
  return cx, cy
end

-- Starts the motion of the medals that go to a new slot. `from[i]` is the number of slots between the slot before and
-- the new slot of medal `i`.
function fan.slideFrom(self, from, time)
  self.slide, self.slideAt, self.time = from, time, time
end

-- True while a medal is on its way to its slot.
function fan.sliding(self) return self.slideAt ~= nil and self.time - self.slideAt < SLIDE_TIME end

local function inside(self, i, x, y)
  local cx, cy = fan.center(self, i)
  return (x - cx) ^ 2 + (y - cy) ^ 2 <= (layout.chip / 2) ^ 2
end

-- The medal at a point, or nil. A medal is above the medals before it. The medal that shows its card is above all.
function fan.at(self, x, y)
  local top = fan.shown(self)
  if top and inside(self, top, x, y) then return top end
  for i = #self.ids, 1, -1 do
    if inside(self, i, x, y) then return i end
  end
  return nil
end

-- The medal that shows its card: the medal under the pointer, else the medal that the player selected.
function fan.shown(self) return self.hovered or self.selected end

-- Selects a medal, or no medal (nil).
function fan.select(self, i, time)
  local before = fan.shown(self)
  self.selected = i
  if fan.shown(self) ~= before then self.shownAt = time end
end

-- Finds the medal under the pointer.
function fan.update(self, x, y, time, dt)
  self.time = time
  local before = fan.shown(self)
  local found = x and fan.at(self, x, y) or nil
  self.hovered = found
  if fan.shown(self) ~= before then self.shownAt = time end
  for i = 1, #self.ids do
    local target = (i == found or i == self.selected) and 1 or 0
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
  if fan.shown(self) == i then
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
  local top = fan.shown(self)
  for i = 1, #self.ids do
    if i ~= top then chip(self, i, flashes, time) end
  end
  if top then chip(self, top, flashes, time) end
end

-- Draws the slots that have no relic: recessed rings, as space for a medal. Draw them before the medals.
function fan.drawSlots(self)
  for i = #self.ids + 1, self.slots do
    local cx, cy = fan.slot(self, i)
    gfx.circle(cx, cy + px(1), SLOT, C.white, 0.1)
    gfx.gradient(function() lg.circle('fill', cx, cy, SLOT, 40) end, cx - SLOT, cy - SLOT, 2 * SLOT, 2 * SLOT, C.wellHi, C.wellLo)
    gfx.ring(cx, cy, SLOT - px(0.5), px(1), C.line)
  end
end

-- Draws the amber ring of the medal that the player selected. Draw it after the medals.
function fan.drawSelection(self)
  local i = self.selected
  if not i then return end
  local cx, cy = fan.center(self, i)
  cy = cy + (self.side == 'enemy' and 1 or -1) * RAISE * self.raise[i]
  local r = layout.chip / 2
  for k = 3, 1, -1 do gfx.ring(cx, cy, r + px(2) + k * px(1.5), px(3), C.accent, 0.07) end
  gfx.ring(cx, cy, r + px(1.5), px(2.5), C.accent)
end

-- The rectangle of the card of the medal under the pointer, or nil. The card of the player is above the fan, and the card
-- of the enemy is below it. Near a side of the stage, the card moves, thus the full card stays in view.
function fan.cardRect(self)
  local shown = fan.shown(self)
  if not shown then return nil end
  local card = layout.card
  local center = self.left + (shown - 1) * layout.fanStep + layout.fanStep / 2
  center = math.max(layout.edge + card.w / 2, math.min(center, layout.stage.w - layout.edge - card.w / 2))
  local y = self.side == 'enemy' and self.area.y + self.area.h + card.gap or self.area.y - card.gap - card.h
  return { x = center - card.w / 2, y = y, w = card.w, h = card.h }
end

-- The opacity and the scale of the card while it comes into view.
function fan.cardEnter(self, time)
  local t = math.min(1, (time - self.shownAt) / TIP_TIME)
  return t, 0.96 + 0.04 * t
end

-- Draws the card of a relic in a rectangle. It has the size of each card of the game, and its text is in the place of the key.
function fan.drawCard(self, r)
  local shown = fan.shown(self)
  local id = self.ids[shown]
  local relic = self.items[shown]
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
