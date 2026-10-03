-- The upgrades. The crowns at the top, the medal board of 16 set
-- slots and the panel of the selected upgrade in the middle, and the key that goes back at the bottom.
local gfx = require('gfx')
local icons = require('icons')
local json = require('json')
local layout = require('layout')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local lg = love.graphics
local C, px = theme.color, gfx.px

local upgrades = {}
upgrades.__index = upgrades

local L = layout.upgrades

local function slotOf(view, i)
  local s = view.slots[i]
  if s == nil or s == json.null then return nil end
  return s
end

function upgrades.new(app, view)
  local self = setmetatable({ app = app, view = view, time = 0, fresh = nil, boughtAt = nil,
    crowns = { from = view.meta.crowns, to = view.meta.crowns, at = -10 } }, upgrades)
  -- The first upgrade that the player can buy is selected, else the first upgrade.
  for i = 1, #view.slots do
    local s = slotOf(view, i)
    if s and not self.selected then self.selected = s.id end
    if s and s.affordable and s.next_cost ~= nil then
      self.selected = s.id
      break
    end
  end
  return self
end

function upgrades:find(id)
  for i = 1, #self.view.slots do
    local s = slotOf(self.view, i)
    if s and s.id == id then return s, i end
  end
end

function upgrades:apply(view, events)
  self.view = view
  for _, e in ipairs(events) do
    if e.type == 'upgrade_bought' then
      self.fresh = { id = e.id, level = e.level }
      self.boughtAt = self.time
      self.crowns = { from = e.crowns_before, to = e.crowns, at = self.time }
    end
  end
end

function upgrades:refused(view) self.view = view end
function upgrades:update(dt) self.time = self.time + dt end
function upgrades:settled() return not self.boughtAt or self.time - self.boughtAt > 0.9 end

function upgrades:hit(x, y)
  for i = 1, #self.view.slots do
    if slotOf(self.view, i) and layout.contains(layout.upgradeSlot(i), x, y) then return 'slot' .. i end
  end
  if layout.contains(L.buy, x, y) then return 'buy' end
  if layout.contains(L.back, x, y) then return 'back' end
end

-- `slot` takes the number of the slot (1 to 16) or the id of an upgrade.
function upgrades:control(name, arg)
  if name == 'slot' then
    if type(arg) == 'string' then
      local _, i = self:find(arg)
      arg = i
    end
    return arg and layout.upgradeSlot(arg)
  end
  if name == 'buy' then return L.buy end
  if name == 'back' then return L.back end
end

function upgrades:canBuy()
  local s = self:find(self.selected)
  return s and s.next_cost ~= nil and s.affordable
end

function upgrades:activate(name)
  if name:find('^slot') then
    local s = slotOf(self.view, tonumber(name:sub(5)))
    if s then self.selected = s.id end
  elseif name == 'buy' then
    if self:canBuy() and not self.app.waiting() then self.app.send({ cmd = 'buy_upgrade', upgrade = self.selected }) end
  elseif name == 'back' then
    if not self.app.waiting() then self.app.send({ cmd = 'back' }) end
  end
end

function upgrades:key(key)
  if key == 'return' or key == 'kpenter' then
    self:activate('buy')
    return true
  end
  return false
end

-- An upgrade with no art gets the plain mark of icons.lua.
local function art(id) return icons.UPGRADE_ART[id] or { kind = 'icon', id = id } end

local function flairOf(s) return s.next_cost == nil and C.accent or C.upgrade end

local function slot(self, i, r, pointer)
  local s = slotOf(self.view, i)
  local radius = px(9)
  local selected = s and s.id == self.selected
  local hover = s and pointer.hover == 'slot' .. i
  if selected then
    local flair = flairOf(s)
    for k = 3, 1, -1 do gfx.rect(r.x - px(2) - k * px(3), r.y - px(2) - k * px(3), r.w + px(4) + k * px(6), r.h + px(4) + k * px(6), radius + k * px(3), C.accent, 0.06) end
    gfx.rect(r.x - px(2), r.y - px(2), r.w + px(4), r.h + px(4), radius + px(2), C.accent)
    gfx.gradientRect(r.x, r.y, r.w, r.h, radius, theme.mix(flair, 0.22, C.raisedHi), C.raisedLo)
    gfx.topLight(r.x, r.y, r.w, r.h, radius, px(1), C.shine)
  elseif hover then
    gfx.gradientRect(r.x, r.y, r.w, r.h, radius, C.wellLo, C.panel)
    gfx.gradientRect(r.x, r.y, r.w, px(6), radius, theme.alpha(C.black, 0.55), theme.alpha(C.black, 0))
  else
    gfx.well(r.x, r.y, r.w, r.h, radius)
  end
  if not s then return end
  local fresh = self.fresh and self.fresh.id == s.id and self.fresh.level or nil
  local since = self.boughtAt and self.time - self.boughtAt
  local far = s.level == 0 and not s.affordable
  local top = r.y + (r.h - 6.02) / 2
  local cx = r.x + r.w / 2
  local flair = flairOf(s)
  local function medal() ui.medal(art(s.id), cx, top + 1.8, 3.6, flair) end
  if far then gfx.muted(0.6, 0.6, medal) else medal() end
  gfx.text(s.name, 'bold', 0.8, r.x, top + 3.9, { color = far and C.dim or C.text, align = 'center', width = r.w, line = 0.92 })
  -- The pips of the levels, and the cost of the next level.
  local pipW = s.max_level * 0.5 + (s.max_level - 1) * 0.22
  local costOpts = { size = 0.8, face = 'display', tracking = 0.03 }
  local costW = s.next_cost ~= nil and 0.45 + ui.amountWidth(s.next_cost, costOpts) or 0
  local x = cx - (pipW + costW) / 2
  local rowY = top + 5.12
  ui.pips(x, rowY + 0.45, s.level, s.max_level, 0.5, 0.22, fresh, since)
  if s.next_cost ~= nil then
    local color = s.affordable and C.accent or C.dim
    costOpts.color, costOpts.icon = color, color
    ui.amount('crowns', s.next_cost, x + pipW + 0.45, rowY, 0.9, costOpts)
  end
end

local function panel(self, pointer)
  local s = self:find(self.selected)
  local p = L.panel
  if not s then
    gfx.raised(p.x, p.y, p.w, p.h, px(12))
    return
  end
  local flair = flairOf(s)
  local since = self.boughtAt and self.time - self.boughtAt
  local scale = 1
  if since and since < 0.8 then scale = 1 + 0.03 * (1 - gfx.ease(since / 0.8)) end
  gfx.scaled(p.x + p.w / 2, p.y + p.h / 2, scale, scale, function()
    if since and since < 0.8 then
      -- The ring of the purchase grows and fades.
      local t = since / 0.8
      local grow = 1 * math.min(1, t * 2)
      gfx.rect(p.x - grow, p.y - grow, p.w + 2 * grow, p.h + 2 * grow, px(12) + grow, C.accent, 0.6 * (1 - math.min(1, t * 2)))
    end
    gfx.shadow(p.x, p.y, p.w, p.h, px(12), px(12), px(14), -px(8), 0.8)
    gfx.rect(p.x, p.y + px(3), p.w, p.h, px(12), C.edge)
    gfx.gradientRect(p.x, p.y, p.w, p.h, px(12), C.raisedHi, C.raisedLo, true)
    gfx.gradientRect(p.x, p.y, p.w, p.h * 0.55, px(12), theme.alpha(flair, 0.24), theme.alpha(flair, 0))
    gfx.outline(p.x, p.y, p.w, p.h, px(12), px(1), theme.mix(flair, 0.45, theme.hex('#111111')))
    gfx.topLight(p.x, p.y, p.w, p.h, px(12), px(1), C.shine)
    gfx.text('UPGRADE', 'bold', 0.68, L.kind.x, L.kind.y, { color = flair, tracking = 0.12, line = L.kind.h })
    ui.medal(art(s.id), L.medal.x, L.medal.y, L.medal.size, flair)
    gfx.text(s.name:upper(), 'display', 1.7, L.name.x, L.name.y, { align = 'center', width = L.name.w, line = L.name.h, tracking = 0.04 })
    -- The pips and the level.
    local fresh = self.fresh and self.fresh.id == s.id and self.fresh.level or nil
    local lv = L.level
    local label = ('Level %d of %d'):format(s.level, s.max_level)
    local pipW = s.max_level * 0.7 + (s.max_level - 1) * 0.3
    local w = pipW + 0.6 + gfx.textWidth(label, 'body', 0.8)
    local x = lv.x + (lv.w - w) / 2
    ui.pips(x, lv.y + lv.h / 2, s.level, s.max_level, 0.7, 0.3, fresh, since)
    gfx.text(label, 'body', 0.8, x + pipW + 0.6, lv.y, { color = C.dim, line = lv.h })
    local lines = gfx.wrap(s.text, 'body', 0.95, L.text.w, true)
    gfx.lines(lines, 'body', 0.95, L.text.x, L.text.y, 0.95 * 1.45, { align = 'center', width = L.text.w })
    -- The cost of each level.
    local lad = L.ladder
    gfx.well(lad.x, lad.y, lad.w, lad.h, px(9))
    for i, cost in ipairs(s.costs) do
      local y = lad.y + 0.5 + (i - 1) * 1.8
      local state = i <= s.level and 'bought' or (i == s.level + 1 and 'next' or '')
      local color = state == 'bought' and C.text or (state == 'next' and C.accent or C.dim)
      local face = state == 'next' and 'bold' or 'body'
      local left, right = lad.x + 0.75, lad.x + lad.w - 0.75
      gfx.text(('Level %d'):format(i), face, 0.85, left, y, { color = color, line = 1.5 })
      local mark = state == 'bought' and 'Bought' or (state == 'next' and 'Next' or '')
      gfx.text(mark, face, 0.85, right - 4.2, y, { color = color, line = 1.5, align = 'right', width = 4.2 })
      local opts = { size = 0.85, face = face, color = color, icon = color }
      local aw = ui.amountWidth(cost, opts)
      ui.amount('crowns', cost, right - 4.2 - 0.6 - aw, y, 1.5, opts)
    end
    local maxed = s.next_cost == nil
    local key = { disabled = maxed or not s.affordable, hover = pointer.hover == 'buy', pressed = pointer.pressed == 'buy' and pointer.hover == 'buy' }
    ui.key(L.buy, maxed and 'Max level' or 'Buy', 'primary', key, { size = 1.2, cost = not maxed and { value = s.next_cost, currency = 'crowns' } or nil })
  end)
  if since and since < 0.8 then
    -- A light goes across the panel after a purchase.
    local t = since / 0.8
    local cx = p.x - p.w + 2 * p.w * gfx.ease(t)
    lg.stencil(function() lg.rectangle('fill', p.x, p.y, p.w, p.h, px(12), px(12), 12) end, 'replace', 1)
    lg.setStencilTest('equal', 1)
    -- linear-gradient(105deg, transparent 40%, white 20% at 50%, transparent 60%): thin slices with a soft profile.
    local slices = 12
    for i = 0, slices - 1 do
      local a0, a1 = 0.4 + 0.2 * i / slices, 0.4 + 0.2 * (i + 1) / slices
      local alpha = 0.2 * (1 - math.abs((i + 0.5) / slices - 0.5) * 2)
      gfx.setColor(C.white, alpha)
      lg.polygon('fill', cx + p.w * a0, p.y, cx + p.w * a1, p.y, cx + p.w * a1 - 3, p.y + p.h, cx + p.w * a0 - 3, p.y + p.h)
    end
    lg.setStencilTest()
  end
end

function upgrades:draw(pointer)
  gfx.text('UPGRADES', 'display', 2.5, L.heading.x, L.heading.y, { tracking = 0.04, line = L.heading.h })
  local pr = L.purse
  gfx.well(pr.x, pr.y, pr.w, pr.h, px(9))
  local opts = { size = 1.35, face = 'display', color = C.accent, tracking = 0.02 }
  local value = ui.counted(self.crowns.from, self.crowns.to, self.time - self.crowns.at)
  local aw = ui.amountWidth(value, opts)
  ui.amount('crowns', value, pr.x + pr.w - 0.75 - aw, pr.y, pr.h, opts)
  local b = L.board
  gfx.shelf(b.x, b.y, b.w, b.h, px(12))
  for i = 1, 16 do slot(self, i, layout.upgradeSlot(i), pointer) end
  panel(self, pointer)
  ui.key(L.back, 'Back', 'key', ui.state(pointer, 'back'))
  gfx.text(text.UPGRADES_HINT, 'body', 0.8, L.hint.x, L.hint.y, { color = C.dim, align = 'right', width = L.hint.w, line = L.hint.h })
end

function upgrades:state()
  local s = self:find(self.selected)
  return { selected = self.selected, canBuy = self:canBuy() and true or false, level = s and s.level, crownsShown = ui.counted(self.crowns.from, self.crowns.to, self.time - self.crowns.at) }
end

return upgrades
