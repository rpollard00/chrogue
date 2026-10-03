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

--[[
  The other screens. The values come from the web game at 1280 by 720, where 1rem is 16 pixels and the window is
  80 by 45 rem. Where the web game gives an area the size of its content, this table gives the area the size of its
  largest content (DESIGN.md, "The first rule: a set layout"). The comments name these areas.
]]

-- The menu of a screen between runs: a raised bar, the primary key with the full width, and two keys in a row.
local function menu(y)
  return {
    bar = rect(24.5, y, 31, 8.8),
    primary = rect(25.6, y + 1.1, 28.8, 3.3),
    left = rect(25.6, y + 5.3, 14.05, 2.25),
    right = rect(40.35, y + 5.3, 14.05, 2.25),
    full = rect(25.6, y + 5.3, 28.8, 2.25),
  }
end

layout.title = {
  wordmark = rect(20, 10.89, 40, 5.4),
  tagline = rect(20, 17.69, 40, 1.5),
  menu = menu(20.58),
  -- The wells have the width of their largest value: 4 digits of crowns, "8 of 8 floors", and 4 digits of runs.
  -- In the web game, each well has the width of its value.
  stats = { y = 30.78, h = 3.33, wells = { rect(28.7, 30.78, 6.6, 3.33), rect(36.0, 30.78, 8.6, 3.33), rect(45.3, 30.78, 6.0, 3.33) } },
}

-- The end of a run. In the web game, the column moves up or down with the number of rows of the tally.
-- Here, the column has the positions of a won run, the largest content, and the tally well keeps its height.
layout.over = {
  emblem = rect(37.98, 6.93, 4.03, 4.5),
  heading = rect(10, 12.83, 60, 3),
  line = rect(20, 17.23, 40, 1.51),
  tally = rect(24.5, 20.14, 31, 7.74),
  rows = { x = 25.5, w = 29, y = { 20.94, 22.69 }, total = 24.79 },
  menu = menu(29.27),
}

layout.upgrades = {
  heading = rect(10, 1.5, 30, 3),
  -- The purse has the width of 3 digits of crowns. In the web game, it has the width of its value.
  purse = rect(64.4, 1.7, 5.6, 2.6),
  board = rect(10, 5.25, 38.25, 34.9),
  boardPad = 0.8, slotGap = 0.6,
  panel = rect(49, 5.25, 21, 34.9),
  kind = rect(50.1, 6.25, 18.8, 1.02),
  medal = { x = 59.5, y = 11.22, size = 6.5 },
  name = rect(50.1, 15.17, 18.8, 1.87),
  level = rect(50.1, 17.74, 18.8, 1.2),
  -- The text of the longest upgrade has two lines. The slot has space for three lines.
  text = rect(50.1, 19.64, 18.8, 4.2),
  -- The ladder has the rows of the upgrade with the most levels (3).
  ladder = rect(50.1, 24.5, 18.8, 6.1),
  buy = rect(50.1, 35.95, 18.8, 3),
  back = rect(10, 41.08, 3.96, 2.25),
  hint = rect(40, 41.66, 30, 1.08),
}

-- The rectangle of a slot of the medal board. `i` starts at 1.
function layout.upgradeSlot(i)
  local u = layout.upgrades
  local b, pad, gap = u.board, u.boardPad, u.slotGap
  local w, h = (b.w - 2 * pad - 3 * gap) / 4, (b.h - 2 * pad - 3 * gap) / 4
  local col, row = (i - 1) % 4, math.floor((i - 1) / 4)
  return rect(b.x + pad + col * (w + gap), b.y + pad + row * (h + gap), w, h)
end

layout.camp = {
  foe = {
    plaque = rect(1, 1.16, 78, 6.5),
    medal = rect(2.1, 2.66, 3.5, 3.5),
    kicker = rect(6.85, 2.56, 13, 0.99),
    name = rect(6.85, 3.55, 13, 1.52),
    sub = rect(6.85, 5.07, 13, 1.2),
    -- The fan has space for the two traits of the last boss. In the web game, the well moves when the enemy has traits.
    fan = rect(20.3, 3.26, 4.05, 2.3),
    -- The well has space for the 16 pieces of the last enemy (16 glyphs of 1.8rem with a gap of 0.1rem, and the padding).
    -- In the web game, it has the width of its pieces.
    well = rect(25.6, 3.11, 28.6, 2.6),
    -- The start key keeps its place when the text below it goes. In the web game, the key moves down.
    start = rect(64.1, 2.25, 13.8, 3),
    reason = rect(61.94, 5.56, 15.96, 1.2),
  },
  reward = rect(1, 8.66, 33.05, 24.5),
  shop = rect(35.05, 8.66, 43.95, 24.5),
  shelfPad = 1, headY = 9.46, headH = 2.4, cardY = 15.14, cardGap = 0.9,
  -- Each shelf has set slots for its largest content: 3 reward cards and 4 shop items. A card keeps its slot when the
  -- player takes or buys a different card. A slot with no card stays as an empty slot.
  rewardSlots = 3, shopSlots = 4,
  skip = rect(24.22, 9.54, 8.83, 2.25),
  reroll = rect(67.62, 9.54, 10.38, 2.25),
  me = {
    plaque = rect(1, 34.16, 78, 9.67),
    medal = rect(2.1, 37.25, 3.5, 3.5),
    army = rect(6.85, 37.1, 12.5, 1.44),
    hint = rect(6.85, 38.74, 12.5, 2.16),
    homes = rect(20.6, 35.06, 30, 7.88),
    kicker = rect(51.85, 37.16, 18, 0.99),
    fan = rect(51.85, 38.55, 18.3, 2.3),
    -- The purse has the width of 4 digits of gold. In the web game, it has the width of its value.
    purse = rect(71.5, 37.7, 6.4, 2.6),
  },
}

-- The home squares of the camp: 0 to 15. Rank 2 is the top row.
local homes = {}
do
  local h = layout.camp.me.homes
  local border = layout.boardBorder
  local size = (h.w - 2 * border) / 8
  for s = 0, 15 do
    local f, r = s % 8, math.floor(s / 8)
    homes[s] = rect(h.x + border + f * size, h.y + border + (1 - r) * size, size, size)
  end
end

function layout.homeSquare(s) return homes[s] end

-- The set slots of the cards of a shelf. The row of slots is in the center of the shelf.
local function slots(shelf, count)
  local c = layout.camp
  local w, gap = 9.75, c.cardGap
  local total = count * w + math.max(0, count - 1) * gap
  local x = shelf.x + (shelf.w - total) / 2
  local list = {}
  for i = 1, count do list[i] = rect(x + (i - 1) * (w + gap), c.cardY, w, 14.25) end
  return list
end

layout.camp.slots = { reward = slots(layout.camp.reward, layout.camp.rewardSlots), shop = slots(layout.camp.shop, layout.camp.shopSlots) }

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
