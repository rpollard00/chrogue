-- The state of the battle screen and its behavior. This module is the port of viewBattle in src/ui/views/battle.ts.
-- It has no drawing. board.lua, plaques.lua, fan.lua and result.lua draw this state.
local rules = require('rules')
local E, B = rules.engine, rules.battle

local screen = {}

-- The delay shows the move of the player before the enemy moves.
local ENEMY_DELAY = 0.35
screen.MOVE_TIME = 0.15
screen.GONE_TIME = 0.26
screen.FLOAT_TIME = 1.0
screen.FLASH_TIME = 1.1
screen.ALARM_TIME = 0.76
screen.INTRO_TIME = 1.9
screen.PROMOTE_TIME = 0.65
screen.PROMOTE_DELAY = 0.18
screen.PIP_TIME = 0.5
screen.COUNT_TIME = 0.6

screen.WIN_TEXT = {
  w = {
    checkmate = 'Checkmate. You won the battle.',
    rout = 'The enemy king is alone. You won the battle.',
    stalemate = 'The enemy has no legal move. You won the battle.',
  },
  b = {
    checkmate = 'Checkmate. The run ends.',
    rout = 'Your king is alone. The run ends.',
    stalemate = 'You have no legal move. The run ends.',
  },
}
screen.DRAW_TEXT = {
  clock = '50 moves passed with no capture and no pawn advance. The battle is a draw.',
  bare = 'Only the kings remain. The battle is a draw.',
}

local function place(s) return s % 8, 7 - math.floor(s / 8) end

-- Gives each piece of the board a sprite. A sprite keeps its piece for the full battle, thus a move is a tween.
local function syncPieces(self)
  local onBoard = {}
  for s = 0, 63 do
    local p = self.state.board[s + 1]
    if p then
      onBoard[p.id] = true
      local col, row = place(s)
      local sprite = self.sprites[p.id]
      if not sprite then
        self.sprites[p.id] = { id = p.id, type = p.type, color = p.color, col = col, row = row, fromCol = col, fromRow = row, movedAt = -1 }
      else
        if sprite.type ~= p.type then
          sprite.type = p.type
          sprite.promotedAt = self.time
        end
        if sprite.col ~= col or sprite.row ~= row then
          sprite.fromCol, sprite.fromRow = sprite.col, sprite.row
          sprite.col, sprite.row, sprite.movedAt = col, row, self.time
        end
      end
    end
  end
  for id, sprite in pairs(self.sprites) do
    if not onBoard[id] then
      sprite.goneAt = self.time
      self.gone[#self.gone + 1] = sprite
      self.sprites[id] = nil
    end
  end
end

function screen.new(makeRun, meta)
  local run = makeRun()
  local battle = B.createBattle(run)
  local self = {
    makeRun = makeRun, meta = meta, run = run, battle = battle, state = battle.state,
    spec = rules.floors.floorOf(run), floors = #rules.floors.FLOORS, scout = rules.upgrades.canScout(meta),
    -- The fields of the view in the web game.
    selected = -1, targets = {}, promotion = nil, last = nil, busy = false,
    time = 0, enemyAt = nil,
    sprites = {}, gone = {}, floaters = {}, flashes = {},
    alarmAt = nil, resultAt = nil, introAt = 0,
    stashAt = { w = nil, b = nil },
    gold = { from = 0, to = 0, at = -1 },
    confirm = false,
    -- The time of the enemy search, in milliseconds.
    search = { count = 0, last = 0, worst = 0, total = 0 },
    moves = 0,
  }
  syncPieces(self)
  return self
end

-- The square of the king in check, or -1.
function screen.checkSquare(self)
  local state = self.state
  return E.inCheck(state, state.turn) and E.kingSquare(state, state.turn) or -1
end

-- The text of the lamp. The lamp is lit when the player can move.
function screen.status(self)
  local text = 'Your move.'
  if self.promotion then text = 'Select the new piece.'
  elseif self.busy then text = 'The enemy thinks'
  elseif E.inCheck(self.state, 'w') then text = 'Your king is in check.' end
  return text, not self.busy and not self.battle.result and self.state.turn == 'w'
end

-- True if the player selected an enemy piece with Scout.
function screen.scouting(self)
  local p = self.state.board[self.selected + 1]
  return p ~= nil and p.color == 'b'
end

-- The moves of the selected piece that go to a square.
function screen.targetsTo(self, s)
  local moves = {}
  for _, m in ipairs(self.targets) do if m.to == s then moves[#moves + 1] = m end end
  return moves
end

local commit

function screen.clickSquare(self, s)
  local state = self.state
  if self.busy or self.battle.result or self.promotion or self.confirm or state.turn ~= 'w' then return end
  local selected, p = state.board[self.selected + 1], state.board[s + 1]
  local own = selected ~= nil and selected.color == 'w'
  local color = p and p.color
  local moves = own and screen.targetsTo(self, s) or {}
  if #moves > 1 then
    self.promotion = moves
  elseif #moves == 1 then
    return commit(self, moves[1])
  elseif (color == 'w' or (color == 'b' and self.scout)) and s ~= self.selected then
    self.selected = s
    self.targets = E.movesFrom(state, s)
  else
    self.selected = -1
    self.targets = {}
  end
end

-- Selects a piece in the promotion picker. `index` starts at 1.
function screen.pickPromotion(self, index)
  local move = self.promotion and self.promotion[index]
  if move then commit(self, move) end
end

function commit(self, move)
  local battle = self.battle
  local mover = self.state.turn
  local report = B.playMove(battle, self.run, move)
  self.last, self.selected, self.targets, self.promotion = move, -1, {}, nil
  self.moves = self.moves + 1
  if battle.result then
    self.resultAt = self.time
  elseif self.state.turn == 'b' then
    self.busy = true
    self.enemyAt = self.time + ENEMY_DELAY
  end
  syncPieces(self)
  if report.capture then
    self.stashAt[mover] = self.time
    -- The gold of a capture shows above the square of the captured piece.
    if report.capture.gold > 0 then
      local col, row = place(report.capture.square)
      self.floaters[#self.floaters + 1] = { col = col, row = row, text = ('+%g'):format(math.floor(report.capture.gold * 10 + 0.5) / 10), at = self.time }
    end
  end
  local gold = math.floor(battle.gold + 0.5)
  if gold ~= self.gold.to then self.gold = { from = self.gold.to, to = gold, at = self.time } end
  if screen.checkSquare(self) >= 0 then self.alarmAt = self.time end
  for _, id in ipairs(report.relics) do self.flashes[id] = self.time end
  return report
end

-- The gold from captures that the purse shows. The number counts to its new value.
function screen.shownGold(self)
  local gold = self.gold
  local t = math.min(1, (self.time - gold.at) / screen.COUNT_TIME)
  return math.floor(gold.from + (gold.to - gold.from) * (1 - (1 - t) ^ 3) + 0.5)
end

function screen.giveUp(self)
  if not self.battle.result then self.confirm = true end
end

function screen.update(self, dt)
  self.time = self.time + dt
  if self.busy and self.time >= self.enemyAt and not self.confirm then
    self.busy = false
    local start = love.timer.getTime()
    local move = B.enemyMove(self.battle, self.run)
    local ms = (love.timer.getTime() - start) * 1000
    local search = self.search
    search.count, search.last, search.total = search.count + 1, ms, search.total + ms
    search.worst = math.max(search.worst, ms)
    commit(self, move)
  end
  for i = #self.gone, 1, -1 do
    if self.time - self.gone[i].goneAt > screen.GONE_TIME then table.remove(self.gone, i) end
  end
  for i = #self.floaters, 1, -1 do
    if self.time - self.floaters[i].at > screen.FLOAT_TIME then table.remove(self.floaters, i) end
  end
end

-- True while a piece moves or the enemy waits. The test script uses it.
function screen.settled(self)
  if self.busy then return false end
  for _, sprite in pairs(self.sprites) do
    if self.time - sprite.movedAt < screen.MOVE_TIME then return false end
  end
  return true
end

return screen
