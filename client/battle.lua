--[[
  The battle screen: its state and its behavior.
  The core has the rules. This module keeps only the state of the interface: the selection, the promotion picker,
  and the motion. Each response of the core gives the full view; the events of the response start the motion.
  board.lua, plaques.lua, fan.lua and result.lua draw this state.
]]
local board = require('board')
local fan = require('fan')
local gfx = require('gfx')
local layout = require('layout')
local plaques = require('plaques')
local result = require('result')
local shaders = require('shaders')
local text = require('text')

local battle = {}
battle.__index = battle

-- The client owns the pause before the enemy move (core/PROTOCOL.md).
battle.ENEMY_DELAY = 0.35
battle.MOVE_TIME = 0.15
battle.GONE_TIME = 0.26
battle.FLOAT_TIME = 1.0
battle.ALARM_TIME = 0.76
battle.INTRO_TIME = 1.9
battle.PROMOTE_TIME = 0.65
battle.PROMOTE_DELAY = 0.18
battle.COUNT_TIME = 0.6
-- The client sends enemy_move again one time after the core refused it. After a second refusal, the lamp tells it.
battle.ENEMY_RETRIES = 1

local function place(s) return s % 8, 7 - math.floor(s / 8) end

-- Gives each piece a sprite. A sprite keeps its piece id for the full battle, thus a move is a tween.
function battle:syncPieces(animate)
  local onBoard = {}
  self.at = {}
  for _, p in ipairs(self.view.pieces) do
    self.at[p.square] = p
    onBoard[p.id] = true
    local col, row = place(p.square)
    local sprite = self.sprites[p.id]
    if not sprite then
      self.sprites[p.id] = { id = p.id, kind = p.kind, color = p.color, col = col, row = row, fromCol = col, fromRow = row, movedAt = -1 }
    else
      -- The light of a promotion comes from the promote event (battle:events), not from this change of kind.
      sprite.kind = p.kind
      if sprite.col ~= col or sprite.row ~= row then
        sprite.fromCol, sprite.fromRow = sprite.col, sprite.row
        sprite.col, sprite.row, sprite.movedAt = col, row, animate and self.time or -1
      end
    end
  end
  for id, sprite in pairs(self.sprites) do
    if not onBoard[id] then
      if animate then
        sprite.goneAt = self.time
        self.gone[#self.gone + 1] = sprite
      end
      self.sprites[id] = nil
    end
  end
end

function battle.new(app, view, events)
  local self = setmetatable({
    app = app, view = view,
    selected = -1, promotion = nil, pending = false,
    time = 0, enemyAt = nil, enemySent = nil, enemyRefusals = 0, enemyFailed = false, givenUp = false,
    sprites = {}, gone = {}, floaters = {}, flashes = {},
    alarmAt = nil, resultAt = nil, introAt = nil,
    stashAt = { w = nil, b = nil },
    gold = { from = view.capture_gold or 0, to = view.capture_gold or 0, at = -1 },
    -- The frames while the core selects the enemy move: the longest frame, in milliseconds.
    enemy = { count = 0, worstFrameMs = 0, lastMs = 0, worstMs = 0 },
  }, battle)
  self:syncPieces(false)
  -- The relics and the traits do not change in a battle. A debug command that changes them starts a new battle screen.
  self.myFan = fan.new(view.relics, 'player', layout.me.fan, nil, view.relic_slots)
  self.foeFan = fan.new(view.traits, 'enemy', layout.foe.fan)
  if view.result then self.resultAt = -10 end
  self:events(events or {})
  self:schedule()
  return self
end

-- Starts the motion of the events of a response.
function battle:events(list)
  for _, e in ipairs(list) do
    local kind = e.type
    if kind == 'battle_start' then
      self.introAt = self.time
    elseif kind == 'capture' then
      -- The side that captured: the color of the captured piece is the other side.
      self.stashAt[e.color == 'b' and 'w' or 'b'] = self.time
      if e.gold and e.gold > 0 then
        local col, row = place(e.square)
        self.floaters[#self.floaters + 1] = { col = col, row = row, text = ('+%g'):format(math.floor(e.gold * 10 + 0.5) / 10), at = self.time }
      end
    elseif kind == 'promote' then
      local sprite = self.sprites[e.id]
      if sprite then sprite.promotedAt = self.time end
    elseif kind == 'check' then
      self.alarmAt = self.time
    elseif kind == 'relic' then
      for _, id in ipairs(e.ids) do self.flashes[id] = self.time end
    elseif kind == 'result' then
      self.resultAt = self.time
    end
  end
end

-- After the move of the player, the enemy waits, then the client asks for the enemy move.
function battle:schedule()
  if self.view.phase == 'enemy' and not self.givenUp and not self.enemyFailed then
    if not self.enemyAt then self.enemyAt = self.time + battle.ENEMY_DELAY end
  else
    self.enemyAt, self.enemySent = nil, nil
  end
end

-- A new view of the same battle.
function battle:apply(view, events, request)
  self.view = view
  self:syncPieces(true)
  self:events(events)
  if request and (request.cmd == 'move' or request.cmd == 'enemy_move') then self.pending = false end
  if request and request.cmd == 'enemy_move' then self.enemyRefusals = 0 end
  if request and request.cmd == 'enemy_move' and self.enemySent then
    local ms = (love.timer.getTime() - self.enemySent) * 1000
    local e = self.enemy
    e.count, e.lastMs, e.worstMs = e.count + 1, ms, math.max(e.worstMs, ms)
    self.enemySent = nil
  end
  local gold = view.capture_gold or 0
  if gold ~= self.gold.to then self.gold = { from = self:shownGold(), to = gold, at = self.time } end
  if view.phase ~= 'player' then self.selected, self.promotion = -1, nil end
  self:schedule()
end

-- A refused command: the view is the view from before the command.
function battle:refused(view, request)
  self.view = view
  self.pending = false
  self:syncPieces(false)
  self.enemySent = nil
  self.enemyAt = nil
  -- If the enemy still has the move, the client asks again after the pause, one time. After a second refusal, the lamp
  -- tells that the enemy cannot move, and Give up stays available.
  if request and request.cmd == 'give_up' then self.givenUp = false end
  if request and request.cmd == 'enemy_move' then
    self.enemyRefusals = self.enemyRefusals + 1
    if self.enemyRefusals > battle.ENEMY_RETRIES then self.enemyFailed = true end
  end
  self:schedule()
end

-- True if the player has the move and no legal move: a battle that starts in checkmate or in stalemate.
function battle:stuck()
  local v = self.view
  return v.phase == 'player' and not v.result and #v.moves == 0
end

-- The square of the king in check, or -1.
function battle:checkSquare() return self.view.check or -1 end

-- The text of the lamp, and true if the lamp is lit. The lamp is lit when the player can move.
function battle:status()
  local v = self.view
  if self:stuck() then return text.STATUS.stuck, false end
  if self.enemyFailed and v.phase == 'enemy' then return text.STATUS.enemyFailed, false end
  local status = text.STATUS.move
  if self.promotion then status = text.STATUS.promotion
  elseif v.phase == 'enemy' then status = text.STATUS.enemy
  elseif v.check and v.turn == 'w' then status = text.STATUS.check end
  return status, v.phase == 'player' and not v.result
end

function battle:busy() return self.view.phase == 'enemy' and not self.enemyFailed end

-- True if the player selected an enemy piece with Scout.
function battle:scouting()
  local p = self.at[self.selected]
  return p ~= nil and p.color == 'b'
end

local NONE = {}

-- The moves of the selected piece, and the same moves by target square. The moves of an enemy piece come from
-- enemy_moves (Scout) and are marks only. The lists change only with the view and the selection, thus they are kept.
function battle:targetSet()
  local cache = self.targetCache
  if cache and cache.view == self.view and cache.selected == self.selected then return cache end
  cache = { view = self.view, selected = self.selected, list = {}, to = {} }
  if self.selected >= 0 then
    local source = self:scouting() and (self.view.enemy_moves or NONE) or self.view.moves
    for _, m in ipairs(source) do
      if m.from == self.selected then
        cache.list[#cache.list + 1] = m
        local at = cache.to[m.to]
        if not at then at = {}; cache.to[m.to] = at end
        at[#at + 1] = m
      end
    end
  end
  self.targetCache = cache
  return cache
end

function battle:targets() return self:targetSet().list end

function battle:targetsTo(s) return self:targetSet().to[s] or NONE end

function battle:commit(move)
  self.selected, self.promotion, self.pending = -1, nil, true
  self.app.send({ cmd = 'move', from = move.from, to = move.to, promo = move.promo })
end

function battle:clickSquare(s)
  local v = self.view
  if v.phase ~= 'player' or v.result or self.promotion or self.pending or self.app.dialog then return end
  local selected, p = self.at[self.selected], self.at[s]
  local own = selected ~= nil and selected.color == 'w'
  local moves = own and self:targetsTo(s) or {}
  if #moves > 1 then
    self.promotion = moves
  elseif #moves == 1 then
    return self:commit(moves[1])
  elseif p and (p.color == 'w' or (p.color == 'b' and v.scout)) and s ~= self.selected then
    self.selected = s
  else
    self.selected = -1
  end
end

-- Selects a piece in the promotion picker. `index` starts at 1.
function battle:pickPromotion(index)
  local move = self.promotion and self.promotion[index]
  if move then self:commit(move) end
end

-- The gold from captures that the purse shows. The number counts to its new value.
function battle:shownGold()
  local gold = self.gold
  local t = math.min(1, (self.time - gold.at) / battle.COUNT_TIME)
  return math.floor(gold.from + (gold.to - gold.from) * (1 - (1 - t) ^ 3) + 0.5)
end

function battle:giveUp()
  if self.view.result then return end
  self.app.confirm(text.GIVE_UP, function()
    -- The pause before the enemy move stops here, thus no enemy_move follows give_up.
    self.givenUp, self.enemyAt = true, nil
    self.app.send({ cmd = 'give_up' })
  end)
end

function battle:update(dt, pointer)
  self.time = self.time + dt
  if self.enemyAt and not self.enemySent and self.time >= self.enemyAt and not self.app.dialog then
    self.enemySent = love.timer.getTime()
    self.pending = true
    self.app.send({ cmd = 'enemy_move' })
  end
  -- The longest frame while the core selects the enemy move.
  if self.enemySent then self.enemy.worstFrameMs = math.max(self.enemy.worstFrameMs, love.timer.getDelta() * 1000) end
  for i = #self.gone, 1, -1 do
    if self.time - self.gone[i].goneAt > battle.GONE_TIME then table.remove(self.gone, i) end
  end
  for i = #self.floaters, 1, -1 do
    if self.time - self.floaters[i].at > battle.FLOAT_TIME then table.remove(self.floaters, i) end
  end
  local x, y = pointer.x, pointer.y
  if self.app.dialog then x, y = nil, nil end
  fan.update(self.myFan, x, y, self.time, dt)
  fan.update(self.foeFan, x, y, self.time, dt)
end

-- True while a piece moves, the enemy waits, or a request waits for the core. The test script uses it.
function battle:settled()
  if self.enemyAt or self.pending then return false end
  for _, sprite in pairs(self.sprites) do
    if self.time - sprite.movedAt < battle.MOVE_TIME then return false end
  end
  return true
end

-- Input

function battle:hit(x, y)
  local panel = result.layout(self)
  if panel then
    if layout.contains(panel.key, x, y) then return 'continue' end
    return nil
  end
  local picker = board.promotionRects(self)
  if picker then
    for i, r in ipairs(picker) do
      if layout.contains(r, x, y) then return 'promo' .. i end
    end
  end
  if not self.view.result and layout.contains(layout.me.giveUp, x, y) then return 'giveUp' end
  local s = layout.squareOf(x, y)
  if s then return 'square' .. s end
  return nil
end

function battle:activate(name)
  if name == 'continue' then
    if not self.app.waiting() then self.app.send({ cmd = 'continue' }) end
  elseif name == 'giveUp' then self:giveUp()
  elseif name:find('^promo') then self:pickPromotion(tonumber(name:sub(6)))
  elseif name:find('^square') then self:clickSquare(tonumber(name:sub(7))) end
end

function battle:key(key)
  if key == 'escape' then
    if self.promotion then self.promotion = nil return true end
    if self.selected >= 0 then self.selected = -1 return true end
  elseif key == 'return' or key == 'kpenter' then
    if result.layout(self) then self:activate('continue') return true end
  end
  return false
end

-- The rectangles of the named controls, for the test script.
function battle:control(name, index)
  if name == 'continue' then return (assert(result.layout(self), 'The battle has no result')).key end
  if name == 'giveUp' then return layout.me.giveUp end
  if name == 'promo' then
    local rects = assert(board.promotionRects(self), 'The promotion picker is not open')
    -- The argument is the number of the item (1 to 4), or the kind of the piece ('q', 'n', 'r', 'b').
    if type(index) == 'string' then
      for i, m in ipairs(self.promotion) do if m.promo == index then return rects[i] end end
      error('The promotion picker has no piece ' .. index)
    end
    return rects[index]
  end
  if name == 'square' then
    local x, y = layout.squareAt(index)
    return { x = x, y = y, w = layout.square, h = layout.square }
  end
  if name == 'stash' then return index == 'enemy' and layout.foe.stash or layout.me.stash end
  if name == 'medal' then
    local f = index[1] == 'enemy' and self.foeFan or self.myFan
    local cx, cy = fan.center(f, index[2])
    return { x = cx - 0.01, y = cy - 0.01, w = 0.02, h = 0.02 }
  end
end

-- Drawing

-- The slots with no relic are below the medals, and they have no foil.
local function drawFan(self, f, flashes, pointer)
  fan.drawSlots(f)
  shaders.foil(fan.bounds(f), self.time, pointer.x, pointer.y, 0.8, function() fan.draw(f, flashes, self.time) end)
end

local function drawCard(self, f, pointer)
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

function battle:draw(pointer)
  plaques.enemy(self, self.foeFan, function(f) drawFan(self, f, nil, pointer) end)
  plaques.player(self, self.myFan, function(f) drawFan(self, f, self.flashes, pointer) end, pointer)
  board.draw(self, pointer)
  result.draw(self, pointer)
  -- The elements above the layout.
  local stash = nil
  if pointer.x and not self.app.dialog then
    if layout.contains(layout.me.stash, pointer.x, pointer.y) then stash = 'player' end
    if layout.contains(layout.foe.stash, pointer.x, pointer.y) then stash = 'enemy' end
  end
  if stash then plaques.stashList(self, stash) end
  drawCard(self, self.foeFan, pointer)
  drawCard(self, self.myFan, pointer)
end

-- The state of the interface, for the dump of the test script.
function battle:state()
  local targets = {}
  for _, m in ipairs(self:targets()) do targets[#targets + 1] = m.to end
  table.sort(targets)
  local status, lit = self:status()
  local promo = {}
  for _, m in ipairs(self.promotion or {}) do promo[#promo + 1] = m.promo end
  return {
    selected = self.selected, targets = targets, scouting = self:scouting(), promotion = promo,
    status = status, lamp = lit, pending = self.pending, waitingForEnemy = self.enemyAt ~= nil,
    shownGold = self:shownGold(), playerMedal = self.myFan.hovered or 0, enemyMedal = self.foeFan.hovered or 0,
    resultShown = result.layout(self) ~= nil, enemyMove = self.enemy, enemyRefusals = self.enemyRefusals,
    enemyFailed = self.enemyFailed, givenUp = self.givenUp, stuck = self:stuck(),
  }
end

return battle
