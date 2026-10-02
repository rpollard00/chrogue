-- The set layout of the battle on a wide screen. The stage is 80 by 45 units, and 1 unit is 1rem of the web game.
-- Each area has a set position and a set size. The content and the state of the battle do not change this table.
-- The values come from the wide battle layout in style.css. A measurement of the web game at 1280 by 720 gave the same values.
local layout = { stage = { w = 80, h = 45 } }

-- 1 CSS pixel in units. The web game has 16 pixels in 1rem at the design size.
function layout.px(n) return n / 16 end

local function rect(x, y, w, h) return { x = x, y = y, w = w, h = h } end

layout.board = rect(24.7, 5.6, 30.6, 30.6)
layout.boardBorder = layout.px(4)
layout.square = (layout.board.w - 2 * layout.boardBorder) / 8

layout.foe = {
  plaque = rect(23.5, 0.75, 33, 4.2),
  medal = rect(24.25, 1.55, 2.6, 2.6),
  name = rect(27.75, 1.49, 18.3, 1.52),
  sub = rect(27.75, 3.01, 18.3, 1.2),
  -- The fan has space for the two traits of the last boss. Its right side is at a set position.
  fan = rect(46.95, 1.7, 3.5, 2.3),
  stash = rect(51.35, 1.55, 4.4, 2.6),
}

layout.me = {
  plaque = rect(23.5, 36.85, 33, 7.4),
  medal = rect(24.25, 37.65, 2.6, 2.6),
  name = rect(27.75, 37.4, 14, 1.52),
  lamp = rect(27.75, 38.92, 14, 1.57),
  -- In the web game, the purse becomes wider when it shows the gold from captures. Here it has the size of its largest content.
  purse = rect(42.75, 37.65, 13, 2.6),
  stash = rect(24.25, 41.1, 4.4, 2.6),
  -- The fan has space for the 10 relics of the game.
  fan = rect(29.55, 41.25, 20.68, 2.3),
  giveUp = rect(51.3, 41.45, 4.45, 1.9),
}

layout.fanStep = 1.75
layout.chip = 2.3
layout.card = { w = 9.75, h = 14.25, gap = 1.5 }
layout.result = { w = 22.5, pad = 1.25 }
layout.edge = layout.px(8)

function layout.contains(r, x, y) return x >= r.x and x < r.x + r.w and y >= r.y and y < r.y + r.h end

-- The top left corner of a square of the board. White is at the bottom.
function layout.squareAt(s)
  local f, r = s % 8, math.floor(s / 8)
  return layout.board.x + layout.boardBorder + f * layout.square, layout.board.y + layout.boardBorder + (7 - r) * layout.square
end

-- The square at a point of the stage, or nil.
function layout.squareOf(x, y)
  local f = math.floor((x - layout.board.x - layout.boardBorder) / layout.square)
  local row = math.floor((y - layout.board.y - layout.boardBorder) / layout.square)
  if f < 0 or f > 7 or row < 0 or row > 7 then return nil end
  return (7 - row) * 8 + f
end

return layout
