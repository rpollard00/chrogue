--[[
  The test hook: `--script <file>` gives a Lua file that returns a list of steps. The steps drive the battle with no person.
  A click goes through love.mousemoved, love.mousepressed and love.mousereleased, thus it uses the input path of a real click.

  { 'click', 'e2' } or { 'click', 12 }     a square, by its name or by its index (0 is a1, 63 is h8)
  { 'press', 'continue' }                   a control: 'continue', 'giveUp', 'ok', 'cancel'
  { 'press', 'promo', 1 }                   an item of the promotion picker: 1 queen, 2 knight, 3 rook, 4 bishop
  { 'key', 'f1' }                           a key
  { 'hover', 'player', 2 }                  a medal of a fan: 'player' or 'enemy', and the number of the medal
  { 'hover', 'stash', 'player' }            a stash
  { 'hover', 'none' }                       the pointer leaves the window
  { 'wait', 0.5 }                           seconds
  { 'settle' }                              until the enemy moved and no piece moves
  { 'screenshot', '/absolute/path.png' }
  { 'dump', '/absolute/path.json' }         the state of the battle
  { 'quit' }
]]
local board = require('board')
local fan = require('fan')
local layout = require('layout')
local result = require('result')
local scenario = require('scenario')
local screen = require('screen')

local script = {}

local function center(r) return r.x + r.w / 2, r.y + r.h / 2 end

-- The point of the stage for a step.
local function target(app, step)
  local kind, a, b = step[1], step[2], step[3]
  if kind == 'click' then
    local s = type(a) == 'string' and scenario.sq(a) or a
    local x, y = layout.squareAt(s)
    return x + layout.square / 2, y + layout.square / 2
  elseif kind == 'press' then
    if a == 'continue' then return center(assert(result.layout(app.screen), 'The battle has no result').key) end
    if a == 'giveUp' then return center(layout.me.giveUp) end
    if a == 'ok' or a == 'cancel' then return center(result.confirmLayout()[a]) end
    if a == 'promo' then return center(assert(board.promotionRects(app.screen), 'The promotion picker is not open')[b]) end
  elseif kind == 'hover' then
    if a == 'stash' then return center(b == 'enemy' and layout.foe.stash or layout.me.stash) end
    return fan.center(a == 'enemy' and app.foeFan or app.myFan, b)
  end
  error('The script has a step that this module does not know: ' .. tostring(kind) .. ' ' .. tostring(a))
end

function script.load(path)
  local chunk = assert(loadfile(path))
  return { steps = chunk(), at = 1, waitUntil = nil, pending = false }
end

-- Does the steps that are ready. Call it one time for each frame.
function script.update(self, app)
  while self.at <= #self.steps do
    local step = self.steps[self.at]
    local kind = step[1]
    if self.pending then return end
    if kind == 'wait' then
      self.waitUntil = self.waitUntil or love.timer.getTime() + step[2]
      if love.timer.getTime() < self.waitUntil then return end
      self.waitUntil = nil
    elseif kind == 'settle' then
      if not screen.settled(app.screen) then return end
    elseif kind == 'click' or kind == 'press' then
      local x, y = app.toWindow(target(app, step))
      love.mousemoved(x, y, 0, 0, false)
      love.mousepressed(x, y, 1, false, 1)
      love.mousereleased(x, y, 1, false, 1)
    elseif kind == 'hover' then
      if step[2] == 'none' then love.mousefocus(false)
      else
        local x, y = app.toWindow(target(app, step))
        love.mousemoved(x, y, 0, 0, false)
      end
    elseif kind == 'key' then
      love.keypressed(step[2], step[2], false)
    elseif kind == 'screenshot' then
      self.pending = true
      love.graphics.captureScreenshot(function(image)
        local file = assert(io.open(step[2], 'wb'))
        file:write(image:encode('png'):getString())
        file:close()
        self.pending = false
      end)
    elseif kind == 'dump' then
      local file = assert(io.open(step[2], 'w'))
      file:write(app.dump())
      file:close()
    elseif kind == 'quit' then
      love.event.quit(0)
      self.at = #self.steps
    else
      error('The script has a step that this module does not know: ' .. tostring(kind))
    end
    self.at = self.at + 1
    -- A step that changes the picture ends the frame, thus the next screenshot shows the change.
    if kind ~= 'wait' and kind ~= 'settle' then return end
  end
end

return script
