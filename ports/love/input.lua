-- The pointer: which control is at a point of the stage, and what a click on it does.
local board = require('board')
local layout = require('layout')
local result = require('result')
local screen = require('screen')

local input = {}

function input.new() return { x = nil, y = nil, hover = nil, pressed = nil, stash = nil } end

-- Returns the name of the control at a point of the stage, or nil. An element above the layout is before the layout.
function input.hit(self, x, y)
  if self.confirm then
    local d = result.confirmLayout()
    if layout.contains(d.ok, x, y) then return 'ok' end
    if layout.contains(d.cancel, x, y) then return 'cancel' end
    return nil
  end
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
  if layout.contains(layout.me.giveUp, x, y) then return 'giveUp' end
  local s = layout.squareOf(x, y)
  if s then return 'square' .. s end
  return nil
end

-- Does the function of a control. `restart` starts the battle again.
function input.activate(self, name, restart)
  if name == 'ok' or name == 'continue' then return restart() end
  if name == 'cancel' then self.confirm = false
  elseif name == 'giveUp' then screen.giveUp(self)
  elseif name:find('^promo') then screen.pickPromotion(self, tonumber(name:sub(6)))
  elseif name:find('^square') then screen.clickSquare(self, tonumber(name:sub(7))) end
end

function input.moved(ui, self, x, y)
  ui.x, ui.y = x, y
  ui.hover = x and input.hit(self, x, y) or nil
  ui.stash = nil
  if x and not self.confirm then
    if layout.contains(layout.me.stash, x, y) then ui.stash = 'player' end
    if layout.contains(layout.foe.stash, x, y) then ui.stash = 'enemy' end
  end
end

function input.pressed(ui, self, x, y)
  input.moved(ui, self, x, y)
  ui.pressed = ui.hover
end

-- A click is a press and a release on the same control.
function input.released(ui, self, x, y, restart)
  local pressed = ui.pressed
  ui.pressed = nil
  input.moved(ui, self, x, y)
  if pressed and pressed == ui.hover then input.activate(self, pressed, restart) end
end

return input
