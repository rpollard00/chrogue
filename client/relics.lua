-- The relics. The crowns at the top, the relic board of 36 set slots and
-- the panel of the selected slot in the middle, and the key that goes back at the bottom. The screen draws only its
-- view: the view has no name and no text of a relic that the player did not unlock.
local gfx = require('gfx')
local icons = require('icons')
local json = require('json')
local layout = require('layout')
local shaders = require('shaders')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local C, px = theme.color, gfx.px

local relics = {}
relics.__index = relics

local L = layout.relics
local LOCK = { kind = 'icon', id = 'lock' }

local function slotOf(view, i)
  local s = view.slots[i]
  if s == nil or s == json.null then return nil end
  return s
end

-- A field of a slot. The core sends `null` for a cost, a feat, or a name that the slot does not have.
local function field(s, name)
  local v = s[name]
  if v == json.null then return nil end
  return v
end

function relics.new(app, view)
  local self = setmetatable({ app = app, view = view, time = 0, fresh = nil, boughtAt = nil, selected = 1,
    crowns = { from = view.meta.crowns, to = view.meta.crowns, at = -10 } }, relics)
  -- The first locked relic that the player can buy is selected, else the first slot.
  for i = 1, #view.slots do
    local s = slotOf(view, i)
    if s and not s.unlocked and s.affordable then
      self.selected = i
      break
    end
  end
  return self
end

function relics:apply(view, events)
  self.view = view
  local bought = false
  for _, e in ipairs(events) do
    if e.type == 'relic_unlocked' then
      bought = true
      self.fresh = self.selected
      self.boughtAt = self.time
      self.crowns = { from = e.crowns_before, to = e.crowns, at = self.time }
    end
  end
  -- A debug command changes the crowns with no event. The purse then shows the crowns of the view at once.
  if not bought and view.meta.crowns ~= self.crowns.to then
    self.crowns = { from = view.meta.crowns, to = view.meta.crowns, at = -10 }
  end
end

function relics:refused(view) self.view = view end
function relics:update(dt) self.time = self.time + dt end
function relics:settled() return not self.boughtAt or self.time - self.boughtAt > 0.9 end

function relics:hit(x, y)
  for i = 1, #self.view.slots do
    if slotOf(self.view, i) and layout.contains(layout.relicSlot(i), x, y) then return 'slot' .. i end
  end
  if layout.contains(L.buy, x, y) then return 'buy' end
  if layout.contains(L.back, x, y) then return 'back' end
end

-- `slot` takes the number of the slot (1 to 36).
function relics:control(name, arg)
  if name == 'slot' then return layout.relicSlot(arg) end
  if name == 'buy' then return L.buy end
  if name == 'back' then return L.back end
end

function relics:canBuy()
  local s = slotOf(self.view, self.selected)
  return s and not s.unlocked and field(s, 'cost') ~= nil and s.affordable
end

function relics:activate(name)
  if name:find('^slot') then
    local i = tonumber(name:sub(5))
    if slotOf(self.view, i) then self.selected = i end
  elseif name == 'buy' then
    -- The core counts the slots from 0.
    if self:canBuy() and not self.app.waiting() then self.app.send({ cmd = 'buy_relic', slot = self.selected - 1 }) end
  elseif name == 'back' then
    if not self.app.waiting() then self.app.send({ cmd = 'back' }) end
  end
end

function relics:key(key)
  if key == 'return' or key == 'kpenter' then
    self:activate('buy')
    return true
  end
  return false
end

-- The medal of a relic that the player did not unlock: the same for each relic.
local function lockMedal(cx, cy, size)
  gfx.muted(0.6, 0.6, function() ui.medal(LOCK, cx, cy, size, C.dim) end)
end

-- The center of the medal of a slot, and the top of its caption row.
local function slotParts(r)
  local top = r.y + (r.h - L.slotMedal - L.slotStep - L.slotCaption) / 2
  return r.x + r.w / 2, top + L.slotMedal / 2, top + L.slotMedal + L.slotStep
end

local function slot(self, i, r, pointer)
  local s = slotOf(self.view, i)
  local selected = s and i == self.selected
  local hover = s and pointer.hover == 'slot' .. i
  ui.slotWell(r, selected and 'selected' or (hover and 'hover' or nil), s and (s.unlocked and C.relic or C.dim))
  if not s or s.unlocked then return end
  local cx, cy, rowY = slotParts(r)
  lockMedal(cx, cy, L.slotMedal)
  local cost = field(s, 'cost')
  if cost then
    local color = s.affordable and C.accent or C.dim
    local opts = { size = 0.95, face = 'display', tracking = 0.03, color = color, icon = color }
    ui.amount('crowns', cost, cx - ui.amountWidth(cost, opts) / 2, rowY, L.slotCaption, opts)
  else
    gfx.setColor(C.dim)
    icons.draw('ribbon', cx, rowY + L.slotCaption / 2, 1.2)
  end
end

-- The medals of the relics that the player unlocked. They have the foil, as the relic medals of a run have.
local function medals(self)
  local since = self.boughtAt and self.time - self.boughtAt
  for i = 1, #self.view.slots do
    local s = slotOf(self.view, i)
    if s and s.unlocked then
      local cx, cy = slotParts(layout.relicSlot(i))
      -- The medal of the relic that the player bought a moment ago grows into view.
      local scale = 1
      if i == self.fresh and since and since < 0.5 then scale = 1 + 0.3 * (1 - gfx.ease(since / 0.5)) end
      gfx.scaled(cx, cy, scale, scale, function() ui.medal({ kind = 'icon', id = s.id }, cx, cy, L.slotMedal, C.relic) end)
    end
  end
end

local function panel(self, pointer)
  local s = slotOf(self.view, self.selected)
  local p = L.panel
  if not s then
    gfx.raised(p.x, p.y, p.w, p.h, px(12))
    return
  end
  local cost, feat = field(s, 'cost'), field(s, 'feat')
  local flair = s.unlocked and C.relic or C.dim
  local since = self.boughtAt and self.time - self.boughtAt
  ui.panel(p, flair, since, function()
    gfx.text(text.KIND.relic:upper(), 'bold', 0.68, L.kind.x, L.kind.y, { color = flair, tracking = 0.12, line = L.kind.h })
    if not s.unlocked then lockMedal(L.medal.x, L.medal.y, L.medal.size) end
    local name = s.unlocked and s.name or text.RELIC_UNKNOWN
    gfx.text(name:upper(), 'display', 1.7, L.name.x, L.name.y, { color = s.unlocked and C.text or C.dim, align = 'center', width = L.name.w, line = L.name.h, tracking = 0.04 })
    local st = L.status
    gfx.text(s.unlocked and text.RELIC_STATUS.unlocked or text.RELIC_STATUS.locked, 'body', 0.8, st.x, st.y,
      { color = s.unlocked and C.relic or C.dim, align = 'center', width = st.w, line = st.h })
    -- The effect of the relic, the condition of its feat, or the line that tells to buy it.
    local body = s.unlocked and s.text or feat or text.RELIC_BUY_FIRST
    local lines = gfx.wrap(body, 'body', 0.95, L.text.w, true)
    gfx.lines(lines, 'body', 0.95, L.text.x, L.text.y, 0.95 * 1.45, { align = 'center', width = L.text.w })
    local label = s.unlocked and text.RELIC_KEY.unlocked or (cost and text.RELIC_KEY.buy or text.RELIC_KEY.feat)
    local buys = not s.unlocked and cost ~= nil
    local key = { disabled = not (buys and s.affordable), hover = pointer.hover == 'buy', pressed = pointer.pressed == 'buy' and pointer.hover == 'buy' }
    ui.key(L.buy, label, 'primary', key, { size = 1.2, cost = buys and { value = cost, currency = 'crowns' } or nil })
  end)
  if s.unlocked then
    -- The medal has the foil, thus it is above the panel. It has the scale of the panel.
    local m, scale = L.medal, ui.panelScale(since)
    local area = { x = m.x - m.size / 2 - 0.5, y = m.y - m.size / 2 - 0.5, w = m.size + 1, h = m.size + 1 }
    shaders.foil(area, self.time, pointer.x, pointer.y, 0.8, function()
      gfx.scaled(p.x + p.w / 2, p.y + p.h / 2, scale, scale, function() ui.medal({ kind = 'icon', id = s.id }, m.x, m.y, m.size, C.relic) end)
    end)
  end
end

function relics:draw(pointer)
  local v = self.view
  gfx.text('RELICS', 'display', 2.5, L.heading.x, L.heading.y, { tracking = 0.04, line = L.heading.h })
  local c = L.count
  gfx.well(c.x, c.y, c.w, c.h, px(9))
  gfx.text(text.relicsUnlocked(v.unlocked, v.total), 'display', 1.35, c.x, c.y, { align = 'center', width = c.w, line = c.h, tracking = 0.02 })
  local pr = L.purse
  gfx.well(pr.x, pr.y, pr.w, pr.h, px(9))
  local opts = { size = 1.35, face = 'display', color = C.accent, tracking = 0.02 }
  local value = ui.counted(self.crowns.from, self.crowns.to, self.time - self.crowns.at)
  local aw = ui.amountWidth(value, opts)
  ui.amount('crowns', value, pr.x + pr.w - 0.75 - aw, pr.y, pr.h, opts)
  local b = L.board
  gfx.shelf(b.x, b.y, b.w, b.h, px(12))
  for i = 1, L.columns * L.columns do slot(self, i, layout.relicSlot(i), pointer) end
  shaders.foil(b, self.time, pointer.x, pointer.y, 0.8, function() medals(self) end)
  panel(self, pointer)
  ui.key(L.back, 'Back', 'key', ui.state(pointer, 'back'))
  gfx.text(text.RELICS_HINT, 'body', 0.8, L.hint.x, L.hint.y, { color = C.dim, align = 'right', width = L.hint.w, line = L.hint.h })
end

function relics:state()
  return { selected = self.selected, canBuy = self:canBuy() and true or false, unlocked = self.view.unlocked,
    crownsShown = ui.counted(self.crowns.from, self.crowns.to, self.time - self.crowns.at) }
end

return relics
