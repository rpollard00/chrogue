--[[
  The camp. One table: the next enemy and the start key at the top, the
  reward shelf and the shop shelf in the middle, and the army, the relics, and the purse at the bottom.
  The core has the rules of the camp. This module keeps the selection of a home square, the selection of a relic medal,
  and the motion of each action.
]]
local fan = require('fan')
local gfx = require('gfx')
local layout = require('layout')
local shaders = require('shaders')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local lg = love.graphics
local C, px = theme.color, gfx.px

local camp = {}
camp.__index = camp

local L = layout.camp
local DEAL_TIME, DEAL_STEP, FLIP_TIME = 0.32, 0.08, 0.36
-- The time that the medal of a discarded relic takes to leave the fan.
local LEAVE_TIME = 0.25

local function sameList(a, b)
  if #a ~= #b then return false end
  for i = 1, #a do if a[i].id ~= b[i].id then return false end end
  return true
end

-- The text of the items of the shop, to see a change of the items that is not a purchase (a debug command).
local function signature(offers)
  local parts = {}
  for i, o in ipairs(offers) do parts[i] = tostring(o.kind) .. ':' .. tostring(o.name) end
  return table.concat(parts, '|')
end

-- The slot of each item of the shop: 1, 2, 3, ... When the player buys an item, the core removes it from the list, and
-- the other items keep their slots.
local function packed(offers)
  local list = {}
  for i = 1, math.min(#offers, L.shopSlots) do list[i] = i end
  return list
end

function camp.new(app, view, events)
  local self = setmetatable({
    app = app, view = view, time = 0, selected = -1,
    dealAt = nil, flipAt = nil, flashes = {}, newUnits = {}, pinned = nil,
    -- The id of the relic whose medal the player selected, or nil. `leaving` has the medals of the discarded relics.
    relic = nil, leaving = {},
    gold = { from = view.gold, to = view.gold, at = -10 },
    shopSlots = packed(view.shop.offers),
  }, camp)
  self:fans()
  self:events(events or {})
  return self
end

function camp:fans()
  local v = self.view
  if not self.myFan or not sameList(self.myFan.items, v.relics) then
    self.myFan = fan.new(v.relics, 'player', L.me.fan, nil, v.relic_slots)
    -- The selection stays on its relic. If the relic is gone, the selection is clear.
    self:selectRelic(self.relic)
  end
  self.myFan.slots = v.relic_slots or 0
  if not self.foeFan or not sameList(self.foeFan.items, v.enemy.traits) then self.foeFan = fan.new(v.enemy.traits, 'enemy', L.foe.fan, 'left') end
end

-- Selects the medal of a relic of the run, or no medal (nil).
function camp:selectRelic(id)
  local index
  for i, relic in ipairs(self.myFan.ids) do if relic == id then index = i end end
  self.relic = index and id or nil
  fan.select(self.myFan, index, self.time)
end

-- The motion of a discard: the medal of each relic of `ids` leaves its place in the fan `old`, and the medals after it
-- go to their new slots.
function camp:leave(old, ids)
  local gone, before, from = {}, {}, {}
  for _, id in ipairs(ids) do gone[id] = true end
  for i, id in ipairs(old.ids) do
    before[id] = i
    if gone[id] then
      local x, y = fan.slot(old, i)
      self.leaving[#self.leaving + 1] = { id = id, x = x, y = y, at = self.time }
    end
  end
  for i, id in ipairs(self.myFan.ids) do from[i] = (before[id] or i) - i end
  fan.slideFrom(self.myFan, from, self.time)
end

function camp:events(list)
  for _, e in ipairs(list) do
    if e.type == 'camp_enter' then
      self.dealAt = self.time
    elseif e.type == 'camp_action' then
      self.gold = { from = e.gold_before, to = e.gold, at = self.time }
      for _, unit in ipairs(e.units or {}) do self.newUnits[unit.id] = self.time end
      for _, id in ipairs(e.relics or {}) do self.flashes[id] = self.time end
      if e.rolled then self.flipAt = self.time end
    end
  end
end

function camp:apply(view, events, request)
  local before = self.view
  self.view = view
  self.cardCache = nil
  local offers = view.shop.offers
  if request and request.cmd == 'buy' and #offers == #before.shop.offers - 1 then
    -- The bought item leaves an empty slot.
    table.remove(self.shopSlots, request.index + 1)
  elseif signature(offers) ~= signature(before.shop.offers) then
    self.shopSlots = packed(offers)
  end
  local old = self.myFan
  self:fans()
  self:events(events)
  for _, e in ipairs(events) do
    if e.type == 'camp_action' and #(e.discarded or {}) > 0 then self:leave(old, e.discarded) end
  end
  -- A change of the gold with no camp action (a debug command) also counts to the new value.
  if self.gold.to ~= view.gold then
    self.gold = { from = tonumber(ui.counted(self.gold.from, self.gold.to, self.time - self.gold.at)), to = view.gold, at = self.time }
  end
end

function camp:refused(view)
  self.view = view
  self.cardCache = nil
end

function camp:update(dt, pointer)
  self.time = self.time + dt
  local x, y = pointer.x, pointer.y
  if self.app.dialog then x, y = nil, nil end
  fan.update(self.myFan, x, y, self.time, dt)
  fan.update(self.foeFan, x, y, self.time, dt)
  for i = #self.leaving, 1, -1 do
    if self.time - self.leaving[i].at > LEAVE_TIME then table.remove(self.leaving, i) end
  end
end

function camp:settled()
  local t = self.time
  return (not self.dealAt or t - self.dealAt > 0.7) and (not self.flipAt or t - self.flipAt > 0.7) and t - self.gold.at > 0.7
    and #self.leaving == 0 and not fan.sliding(self.myFan)
end

-- The cards of a shelf: a list of { rect, face, offer, name of the key, name of the info button }. The list changes only
-- with the view, thus it is kept until the next view.
function camp:cards(shelf)
  self.cardCache = self.cardCache or {}
  local list = self.cardCache[shelf]
  if not list then
    list = self:makeCards(shelf)
    self.cardCache[shelf] = list
  end
  return list
end

function camp:makeCards(shelf)
  local v = self.view
  local list = {}
  if shelf == 'reward' then
    local reward = v.reward
    if not reward then return list end
    local rects = L.slots.reward
    for i, offer in ipairs(reward.offers) do
      local face = { kind = offer.kind, art = ui.offerArt(offer), name = offer.name, text = offer.text }
      if reward.open then
        face.verb, face.stamp = 'Take', offer.blocked and text.BLOCKED[offer.blocked] or nil
        face.disabled = offer.blocked ~= nil
      else
        local taken = reward.taken ~= nil and reward.taken + 1 == i
        face.verb, face.settled, face.disabled = taken and 'Taken' or 'Take', taken and 'chosen' or 'passed', true
      end
      if rects[i] then list[i] = { rect = rects[i], slot = i, face = face, offer = offer, key = 'take' .. i, info = 'infoR' .. i } end
    end
  else
    local offers = v.shop.offers
    for i, offer in ipairs(offers) do
      local slot = self.shopSlots[i]
      local face = { kind = offer.kind, art = ui.offerArt(offer), name = offer.name, text = offer.text, verb = 'Buy',
        cost = { value = offer.price, currency = 'gold' }, stamp = offer.blocked and text.BLOCKED[offer.blocked] or nil }
      face.disabled = offer.blocked ~= nil or not offer.affordable
      if slot then
        list[i] = { rect = L.slots.shop[slot], slot = slot, face = face, offer = offer, key = 'buy' .. i, info = 'infoS' .. i }
      end
    end
  end
  return list
end

local function allCards(self)
  local cache = self.cardCache and self.cardCache.all
  if cache then return cache end
  local list = {}
  for _, c in ipairs(self:cards('reward')) do list[#list + 1] = c end
  for _, c in ipairs(self:cards('shop')) do list[#list + 1] = c end
  self.cardCache.all = list
  return list
end

-- True if a slot of a shelf has no card.
local function emptySlot(self, shelf, slot)
  for _, c in ipairs(self:cards(shelf)) do
    if c.slot == slot then return false end
  end
  return true
end

-- Input

function camp:hit(x, y)
  local v = self.view
  -- The card of the selected medal stays in view, and it can be above a card of a shelf. A click on it does not go to the
  -- card below it.
  local card = self.relic and fan.cardRect(self.myFan)
  if card and layout.contains(card, x, y) then return 'away' end
  for _, c in ipairs(allCards(self)) do
    if c.face.text and layout.contains(ui.cardInfo(c.rect), x, y) then return c.info end
    if layout.contains(ui.cardKey(c.rect), x, y) then return c.key end
  end
  if v.reward and v.reward.open and layout.contains(L.skip, x, y) then return 'skip' end
  if layout.contains(L.reroll, x, y) then return 'reroll' end
  if layout.contains(L.foe.start, x, y) then return 'start' end
  for s = 0, 15 do
    if layout.contains(layout.homeSquare(s), x, y) then return 'home' .. s end
  end
  local medal = fan.at(self.myFan, x, y)
  if medal then return 'medal' .. medal end
  if layout.contains(L.me.discard, x, y) then return 'discard' end
  -- While a medal is selected, a click on each other place clears the selection.
  if self.relic then return 'away' end
end

function camp:control(name, arg)
  if name == 'take' or name == 'buy' then
    local c = self:cards(name == 'take' and 'reward' or 'shop')[arg]
    return c and ui.cardKey(c.rect)
  end
  if name == 'card' then
    local c = self:cards(arg[1])[arg[2]]
    return c and c.rect
  end
  if name == 'info' then
    local c = self:cards(arg[1])[arg[2]]
    return c and ui.cardInfo(c.rect)
  end
  if name == 'skip' then return L.skip end
  if name == 'reroll' then return L.reroll end
  if name == 'start' then return L.foe.start end
  if name == 'discard' then return L.me.discard end
  if name == 'home' then return layout.homeSquare(arg) end
  if name == 'medal' then
    local f = arg[1] == 'enemy' and self.foeFan or self.myFan
    local cx, cy = fan.center(f, arg[2])
    return { x = cx - 0.01, y = cy - 0.01, w = 0.02, h = 0.02 }
  end
end

local function unitAt(self, s)
  for _, unit in ipairs(self.view.army) do if unit.home == s then return unit end end
end

function camp:clickHome(s)
  if self.selected < 0 then
    if unitAt(self, s) then self.selected = s end
    return
  end
  local unit = unitAt(self, self.selected)
  local from = self.selected
  self.selected = -1
  if unit and s ~= from then self.app.send({ cmd = 'place', unit = unit.id, square = s }) end
end

-- Asks before the relic of the selected medal leaves the run. The selection stays while the dialog is open.
function camp:discard()
  local app, id = self.app, self.relic
  if not id or app.waiting() then return end
  local relic = self.myFan.items[self.myFan.selected]
  app.confirm(text.discard(relic.name), function() app.send({ cmd = 'discard_relic', relic = id }) end)
end

function camp:activate(name)
  local v, app = self.view, self.app
  local pinned = self.pinned
  self.pinned = nil
  if name == 'discard' then return self:discard() end
  -- A click on each other control clears the selection of a medal. A click on a new medal selects it.
  local relic = self.relic
  self:selectRelic(nil)
  if name == 'away' then return end
  if name:find('^medal') then
    local id = self.myFan.ids[tonumber(name:sub(6))]
    if id ~= relic then
      self.selected = -1
      self:selectRelic(id)
    end
    return
  end
  if name:find('^info') then
    if pinned ~= name then self.pinned = name end
    return
  end
  if name:find('^home') then return self:clickHome(tonumber(name:sub(5))) end
  if app.waiting() then return end
  if name:find('^take') then
    local i = tonumber(name:sub(5))
    local offer = v.reward and v.reward.open and v.reward.offers[i]
    if offer and not offer.blocked then app.send({ cmd = 'take_reward', index = i - 1 }) end
  elseif name:find('^buy') then
    local i = tonumber(name:sub(4))
    local offer = v.shop.offers[i]
    if offer and not offer.blocked and offer.affordable then app.send({ cmd = 'buy', index = i - 1 }) end
  elseif name == 'skip' then
    if v.reward and v.reward.open then app.send({ cmd = 'skip_reward' }) end
  elseif name == 'reroll' then
    if v.shop.can_reroll then app.send({ cmd = 'reroll' }) end
  elseif name == 'start' then
    if v.can_start then app.send({ cmd = 'start_battle' }) end
  end
end

function camp:key(key)
  if key == 'escape' then
    if self.pinned then self.pinned = nil return true end
    if self.relic then self:selectRelic(nil) return true end
    if self.selected >= 0 then self.selected = -1 return true end
  elseif key == 'return' or key == 'kpenter' then
    self:activate('start')
    return true
  end
  return false
end

-- Drawing

local function kingMedal(r, color, flair)
  local cx, cy = r.x + r.w / 2, r.y + r.h / 2
  gfx.medal(cx, cy, r.w, flair, true)
  gfx.piece('k', color, cx, cy, r.w * 0.56 * 1.2)
end

local function bossBadge(x, y)
  local bh = 1.04
  local bw = gfx.textWidth('BOSS', 'display', 0.72, 0.12) + 0.8 - 0.12 * 0.72
  gfx.rect(x, y + px(1), bw, bh, px(3), C.bossEdge)
  gfx.gradientRect(x, y, bw, bh, px(3), C.bossHi, C.danger)
  gfx.text('BOSS', 'display', 0.72, x + 0.4, y, { color = C.bossInk, tracking = 0.12, line = bh })
end

local function drawFan(self, f, flashes, pointer)
  shaders.foil(fan.bounds(f), self.time, pointer.x, pointer.y, 0.8, function() fan.draw(f, flashes, self.time) end)
end

local function enemyPlaque(self, pointer)
  local v, F = self.view, L.foe
  local p = F.plaque
  gfx.raised(p.x, p.y, p.w, p.h, px(12))
  kingMedal(F.medal, 'b', C.danger)
  gfx.text('NEXT ENEMY', 'bold', 0.66, F.kicker.x, F.kicker.y, { color = C.dim, tracking = 0.12, line = F.kicker.h })
  gfx.text(v.enemy.name:upper(), 'display', 1.45, F.name.x, F.name.y, { tracking = 0.035, line = F.name.h, shadow = C.shadeSoft })
  local width = gfx.text(text.floor(v.floor), 'body', 0.8, F.sub.x, F.sub.y, { color = C.dim, line = F.sub.h })
  if v.floor.boss then bossBadge(F.sub.x + width + 0.45, F.sub.y + 0.08) end
  drawFan(self, self.foeFan, nil, pointer)
  -- The pieces of the next enemy: the king first, then by value.
  local w = F.well
  gfx.well(w.x, w.y, w.w, w.h, px(9), true)
  local step = gfx.textWidth('♟', 'piece', 1.8) + 0.1
  local x = w.x + 0.75
  for _, kind in ipairs(v.enemy.kinds) do
    local pw = gfx.textWidth('♟', 'piece', 1.8)
    gfx.piece(kind, 'b', x + pw / 2, w.y + w.h / 2, 1.8)
    x = x + step
  end
  ui.key(F.start, 'Start the battle', 'primary', ui.state(pointer, 'start', not v.can_start))
  if v.reward and v.reward.open then
    gfx.text(text.START_REASON, 'body', 0.8, F.reason.x, F.reason.y, { color = C.dim, align = 'right', width = F.reason.w, line = F.reason.h })
  end
end

-- The motion of a card when it comes into view: the deal at the arrival, and the turn of the shop cards after a reroll.
local function cardMotion(self, shelf, i)
  if shelf == 'shop' and self.flipAt then
    local t = (self.time - self.flipAt - (i - 1) * DEAL_STEP) / FLIP_TIME
    if t < 1 then
      local e = gfx.ease(math.max(0, t))
      return math.max(0, math.min(1, t * 1.5)), 0, math.cos(math.rad(80 * (1 - e)))
    end
  elseif self.dealAt then
    local t = (self.time - self.dealAt - (i - 1) * DEAL_STEP) / DEAL_TIME
    if t < 1 then
      local e = gfx.ease(math.max(0, t))
      return math.max(0, math.min(1, t)), 0.8 * (1 - e), 1
    end
  end
  return 1, 0, 1
end

-- A slot with no card: a recessed place of the size of a card.
local function emptySlots(self, shelf)
  for slot, r in ipairs(L.slots[shelf]) do
    if emptySlot(self, shelf, slot) then
      gfx.well(r.x, r.y, r.w, r.h, px(10))
      gfx.outline(r.x, r.y, r.w, r.h, px(10), px(1), C.line, 0.45)
    end
  end
end

local function shelfCards(self, shelf, pointer)
  emptySlots(self, shelf)
  for i, c in ipairs(self:cards(shelf)) do
    local alpha, dy, sx = cardMotion(self, shelf, i)
    if alpha > 0 then
      local r = c.rect
      local st = {
        key = ui.state(pointer, c.key, c.face.disabled),
        info = pointer.hover == c.info or self.pinned == c.info,
        hover = pointer.x and layout.contains(r, pointer.x, pointer.y),
      }
      local function draw()
        lg.push()
        lg.translate(0, dy)
        gfx.scaled(r.x + r.w / 2, r.y + r.h / 2, sx, 1, function()
          gfx.withAlpha(alpha, function() ui.card(r, c.face, st) end)
        end)
        lg.pop()
      end
      -- A relic card has the foil, as the relic medals have.
      if c.face.kind == 'relic' and not c.face.settled and not c.face.stamp then
        shaders.foil({ x = r.x - 0.6, y = r.y - 0.8, w = r.w + 1.2, h = r.h + 1.8 }, self.time, pointer.x, pointer.y, 0.55, draw)
      else
        draw()
      end
    end
  end
end

local function shelves(self, pointer)
  local v = self.view
  -- The reward shelf is on each visit, thus the table has one layout. A visit with no reward has an empty shelf.
  local r = L.reward
  gfx.shelf(r.x, r.y, r.w, r.h, px(12))
  local reward = v.reward
  local head = reward and text.REWARD_HEAD[reward.state] or 'Reward'
  local hw = gfx.text(head:upper(), 'display', 1.2, r.x + L.shelfPad, L.headY, { tracking = 0.04, line = L.headH })
  if reward and reward.open then ui.key(L.skip, 'Skip the reward', 'key', ui.state(pointer, 'skip')) end
  -- With no card, the slots stay empty, and the line of the heading tells why.
  if not reward then
    gfx.text('No reward to select.', 'body', 0.85, r.x + L.shelfPad + hw + 0.8, L.headY, { color = C.dim, line = L.headH })
  end
  shelfCards(self, 'reward', pointer)
  local s = L.shop
  gfx.shelf(s.x, s.y, s.w, s.h, px(12))
  local sw = gfx.text('SHOP', 'display', 1.2, s.x + L.shelfPad, L.headY, { tracking = 0.04, line = L.headH })
  ui.key(L.reroll, 'Get new items', 'key', ui.state(pointer, 'reroll', not v.shop.can_reroll),
    { cost = { value = v.shop.reroll_cost, currency = 'gold' } })
  if #v.shop.offers == 0 then
    gfx.text('The shop is empty.', 'body', 0.85, s.x + L.shelfPad + sw + 0.8, L.headY, { color = C.dim, line = L.headH })
  end
  shelfCards(self, 'shop', pointer)
end

local function homes(self)
  local h = L.me.homes
  gfx.rect(h.x, h.y, h.w, h.h, px(4), C.frame)
  for s = 0, 15 do
    local r = layout.homeSquare(s)
    local f, rank = s % 8, math.floor(s / 8)
    gfx.rect(r.x, r.y, r.w, r.h, 0, (f + rank) % 2 == 1 and C.light or C.dark)
    if s == self.selected then gfx.rect(r.x, r.y, r.w, r.h, 0, C.markSelected) end
  end
  local size = 0.095 * (h.w - 2 * layout.boardBorder)
  for _, unit in ipairs(self.view.army) do
    local r = layout.homeSquare(unit.home)
    local scale = 1
    local added = self.newUnits[unit.id]
    if added then
      -- A new unit grows into its square.
      local t = (self.time - added - 0.1) / 0.5
      if t < 0 then scale = 0 elseif t < 0.6 then scale = 1.7 * t / 0.6 elseif t < 1 then scale = 1.7 - 0.7 * (t - 0.6) / 0.4 end
    end
    if scale > 0.01 then gfx.piece(unit.kind, 'w', r.x + r.w / 2, r.y + r.h / 2, size, 1, scale) end
  end
end

local function playerPlaque(self, pointer)
  local v, M = self.view, L.me
  local p = M.plaque
  gfx.raised(p.x, p.y, p.w, p.h, px(12))
  kingMedal(M.medal, 'w', C.medalRim)
  local x = M.army.x
  x = x + gfx.text('YOUR ARMY ', 'display', 1.2, x, M.army.y, { tracking = 0.04, line = M.army.h })
  gfx.text(('(%d OF %d)'):format(#v.army, v.army_max), 'display', 1.2, x, M.army.y, { color = C.dim, tracking = 0.04, line = M.army.h })
  local lines = gfx.wrap(text.ARMY_HINT, 'body', 0.8, M.hint.w)
  gfx.lines(lines, 'body', 0.8, M.hint.x, M.hint.y, 1.08, { color = C.dim })
  homes(self)
  fan.drawSlots(self.myFan)
  -- The medal of a discarded relic becomes smaller and goes out of view at its place.
  for _, medal in ipairs(self.leaving) do
    local t = (self.time - medal.at) / LEAVE_TIME
    gfx.scaled(medal.x, medal.y, 1 - 0.5 * gfx.ease(t), nil, function()
      gfx.withAlpha(1 - gfx.ease(t), function() ui.medal({ kind = 'icon', id = medal.id }, medal.x, medal.y, 1.9, C.relic) end)
    end)
  end
  drawFan(self, self.myFan, self.flashes, pointer)
  fan.drawSelection(self.myFan)
  -- The Discard key is always there. The player can use it while a medal is selected.
  local off = self.relic == nil
  local function key()
    ui.key(M.discard, text.DISCARD, 'quiet', ui.state(pointer, 'discard', off),
      { color = not off and C.text or nil, hoverColor = theme.mix(C.danger, 0.6, C.text) })
  end
  if off then gfx.withAlpha(0.5, key) else key() end
  local pr = M.purse
  gfx.well(pr.x, pr.y, pr.w, pr.h, px(9))
  local opts = { size = 1.35, face = 'display', color = C.gold, tracking = 0.02 }
  local value = ui.counted(self.gold.from, self.gold.to, self.time - self.gold.at)
  local aw = ui.amountWidth(value, opts)
  ui.amount('gold', value, pr.x + pr.w - 0.75 - aw, pr.y, pr.h, opts)
end

-- The kicker of the fan: the count of the relics and of the relic slots. The count is amber when each slot has a relic.
local function relicKicker(self)
  local v, k = self.view, L.me.kicker
  local slots = v.relic_slots or 0
  local opts = { color = C.dim, tracking = 0.12, line = k.h }
  local width = gfx.text(text.RELICS:upper() .. ' ', 'bold', 0.66, k.x, k.y, opts)
  opts.color = #v.relics >= slots and C.accent or C.dim
  gfx.text(text.relicCount(#v.relics, slots):upper(), 'bold', 0.66, k.x + width, k.y, opts)
end

local function drawRelicCard(self, f, pointer)
  local r = fan.cardRect(f)
  if not r then return end
  local alpha, scale = fan.cardEnter(f, self.time)
  local area = { x = r.x - 0.5, y = r.y - 0.5, w = r.w + 1, h = r.h + 1.2 }
  shaders.foil(area, self.time, pointer.x, pointer.y, 1, function()
    gfx.scaled(r.x + r.w / 2, r.y + r.h / 2, scale, scale, function()
      gfx.withAlpha(alpha, function() fan.drawCard(f, r) end)
    end)
  end)
end

function camp:draw(pointer)
  enemyPlaque(self, pointer)
  shelves(self, pointer)
  playerPlaque(self, pointer)
  -- The elements above the layout: the paper tip of an info button, and the cards of the relics and of the traits.
  for _, c in ipairs(allCards(self)) do
    if c.face.text and (pointer.hover == c.info or self.pinned == c.info) then
      ui.tip(ui.cardInfo(c.rect), c.face.name, c.face.text, layout.stage.w)
    end
  end
  drawRelicCard(self, self.foeFan, pointer)
  drawRelicCard(self, self.myFan, pointer)
  -- The card of a selected medal stays in view. The kicker is below the card, and it is drawn after the card, thus the
  -- shadow of the card does not hide the count.
  relicKicker(self)
end

function camp:state()
  local tip = nil
  for _, c in ipairs(allCards(self)) do
    if self.pinned == c.info or self.app.pointer.hover == c.info then tip = c.face.name end
  end
  return { selected = self.selected, relic = self.relic or false, tip = tip or false, shownGold = tonumber(ui.counted(self.gold.from, self.gold.to, self.time - self.gold.at)),
    playerMedal = self.myFan.hovered or 0, enemyMedal = self.foeFan.hovered or 0, shopSlots = self.shopSlots }
end

return camp
