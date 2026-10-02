-- The elements that more than one screen uses: the port of src/ui/widgets.ts, src/ui/effects.ts and src/ui/tip.ts.
-- Each position and each size is in stage units (1 unit is 1rem of the web game).
local gfx = require('gfx')
local icons = require('icons')
local theme = require('theme')
local text = require('text')
local lg = love.graphics
local C, px = theme.color, gfx.px

local ui = {}

ui.COUNT_TIME = 0.6
ui.ROW_STEP = 0.26

local FLAIR = { piece = C.unit, relic = C.relic, gold = C.gold }
ui.FLAIR = FLAIR

-- The state of the pointer for a control: { hover = bool, pressed = bool }.
function ui.state(pointer, name, disabled)
  return { hover = pointer.hover == name, pressed = pointer.pressed == name and pointer.hover == name, disabled = disabled }
end

-- A number that counts from `from` to `to` in COUNT_TIME seconds after `since` seconds. The text has no decimals.
function ui.counted(from, to, since)
  local t = math.max(0, math.min(1, since / ui.COUNT_TIME))
  return tostring(math.floor(from + (to - from) * (1 - (1 - t) ^ 3) + 0.5))
end

--[[
  Draws an amount: the icon of the currency and the number. y is the top of the line box, `line` its height.
  opts: face, size, color (of the number), icon (the color of the icon), tracking. Returns the width.
]]
function ui.amount(currency, value, x, y, line, opts)
  local size = opts.size
  local iconColor = opts.icon or (currency == 'crowns' and C.accent or C.gold)
  gfx.setColor(iconColor)
  icons.currency(currency, x + size / 2, y + line / 2, size)
  local gap = 0.3 * size
  local w = gfx.text(tostring(value), opts.face or 'display', size, x + size + gap, y, { color = opts.color or C.text, line = line, tracking = opts.tracking })
  return size + gap + w
end

function ui.amountWidth(value, opts)
  return opts.size * 1.3 + gfx.textWidth(tostring(value), opts.face or 'display', opts.size, opts.tracking)
end

local KEY_FONT = {
  key = { face = 'semibold', size = 0.9 },
  quiet = { face = 'body', size = 0.82 },
  primary = { face = 'display', size = 1.3, tracking = 0.05, upper = true },
}

--[[
  A key with its label. `kind` is 'key', 'quiet', or 'primary'. opts: cost = { value, currency }, size, color (label),
  hoverColor. The label is in the center. A cost follows the label, as actionButton in src/ui/widgets.ts.
]]
function ui.key(r, label, kind, state, opts)
  opts = opts or {}
  local f = KEY_FONT[kind]
  local size = opts.size or f.size
  local down, ink = gfx.key(r.x, r.y, r.w, r.h, kind, state)
  if opts.color then ink = opts.color end
  if opts.hoverColor and state.hover and not state.disabled then ink = opts.hoverColor end
  if f.upper then label = label:upper() end
  local width = gfx.textWidth(label, f.face, size, f.tracking)
  local costOpts
  if opts.cost then
    costOpts = { size = size, face = f.face, color = ink, tracking = f.tracking,
      icon = (state.disabled or kind == 'primary') and ink or nil }
    width = width + 0.45 + ui.amountWidth(opts.cost.value, costOpts)
  end
  local x = r.x + (r.w - width) / 2
  x = x + gfx.text(label, f.face, size, x, r.y + down, { color = ink, line = r.h, tracking = f.tracking })
  if opts.cost then ui.amount(opts.cost.currency, opts.cost.value, x + 0.45, r.y + down, r.h, costOpts) end
end

-- The picture of a card or of a relic: a white piece, or an icon, on a medal. `art` is { kind = 'piece', type } or { kind = 'icon', id }.
function ui.medal(art, cx, cy, size, flair, alpha)
  gfx.medal(cx, cy, size, flair)
  if art.kind == 'piece' then
    gfx.piece(art.type, 'w', cx, cy, size * 0.56 * 1.2, alpha)
  else
    gfx.setColor(flair)
    icons.draw(art.id, cx, cy, size * 0.56)
  end
end

-- The art of an offer of the core (an offer of a camp view).
function ui.offerArt(offer)
  if offer.kind == 'piece' then return { kind = 'piece', type = offer.piece } end
  if offer.kind == 'relic' then return { kind = 'icon', id = offer.id } end
  return { kind = 'icon', id = 'coins' }
end

ui.CARD = { w = 9.75, h = 14.25 }

-- The rectangles of the parts of a card that take a click: the key and the info button.
function ui.cardKey(r) return { x = r.x + 0.75, y = r.y + 11.4, w = 8.25, h = 2 } end
function ui.cardInfo(r) return { x = r.x + r.w - 2, y = r.y + 0.65, w = 1.25, h = 1.25 } end

-- The info button: a small paper disk with an "i".
local function infoButton(r, hover)
  local cx, cy, radius = r.x + r.w / 2, r.y + r.h / 2, r.w / 2
  gfx.circle(cx, cy + px(1), radius, C.black, 0.6)
  gfx.radial(cx, cy, radius, hover and theme.mix(C.infoHi, 0.85, C.white) or C.infoHi, hover and theme.mix(C.infoLo, 0.85, C.white) or C.infoLo)
  lg.push()
  lg.translate(cx, cy)
  lg.shear(-0.2, 0)
  gfx.text('i', 'bold', 0.85, -1, -r.h / 2, { color = C.infoInk, align = 'center', width = 2, line = r.h })
  lg.pop()
end

--[[
  The one card of the game (card in src/ui/widgets.ts). face: kind, art, name, text (nil for no info button), stamp,
  settled ('chosen' or 'passed'), verb, cost, disabled. st: key (state of the key), info (true if the pointer is on the
  info button), hover (the pointer is on the card).
]]
function ui.card(r, face, st)
  local flair = FLAIR[face.kind]
  local x, y, w, h = r.x, r.y, r.w, r.h
  local lift = (st.hover and not face.stamp and not face.settled) and 1 or 0
  lg.push()
  if lift > 0 then
    lg.translate(x + w / 2, y + h / 2 - 0.3)
    lg.rotate(math.rad(-0.6))
    lg.translate(-(x + w / 2), -(y + h / 2))
  end
  gfx.shadow(x, y, w, h, px(10), lift > 0 and px(20) or px(12), lift > 0 and px(18) or px(14), -px(8 + 2 * lift), 0.8)
  gfx.raised(x, y, w, h, px(10), { tint = flair, rim = theme.mix(flair, lift > 0 and 0.7 or 0.45, theme.hex('#111111')) })
  gfx.outline(x + 0.3, y + 0.3, w - 0.6, h - 0.6, px(7), px(1), flair, 0.35)
  local function body()
    gfx.text(text.KIND[face.kind]:upper(), 'bold', 0.68, x + 0.85, y + 0.65, { color = flair, tracking = 0.12, line = 1.25 })
    ui.medal(face.art, x + w / 2, y + 4.75, 4.6, flair)
    local lines = gfx.wrap(face.name, 'bold', 0.95, w - 1.1)
    local top = y + 7.44 + (2.18 - #lines * 1.09) / 2
    gfx.lines(lines, 'bold', 0.95, x, top, 1.09, { align = 'center', width = w })
  end
  local function info() if face.text then infoButton(ui.cardInfo(r), st.info) end end
  local function key()
    local k = ui.cardKey(r)
    local chosen = face.settled == 'chosen'
    -- A disabled key has the dim color. The key of the card that the player took has the amber color.
    ui.key(k, face.verb, 'key', st.key, { cost = face.cost, color = chosen and C.accent or nil })
  end
  if face.settled == 'passed' then
    gfx.muted(0.8, 0.45, function() body(); info(); key() end)
  elseif face.stamp then
    gfx.muted(0.8, 0.55, function() body(); key() end)
    info()
  else
    body(); info(); key()
  end
  if face.stamp then
    local label = face.stamp:upper()
    local tw = gfx.textWidth(label, 'bold', 0.8, 0.1) + 1
    lg.push()
    lg.translate(x + w / 2, y + 3.9 + 0.7)
    lg.rotate(math.rad(-9))
    gfx.rect(-tw / 2, -0.7, tw, 1.4, px(4), theme.hex('#15171c'), 0.85)
    gfx.outline(-tw / 2, -0.7, tw, 1.4, px(4), px(2), C.danger)
    gfx.text(label, 'bold', 0.8, -tw / 2, -0.7, { color = C.danger, tracking = 0.1, align = 'center', width = tw, line = 1.4 })
    lg.pop()
  end
  lg.pop()
end

-- The paper tip of an info button: a title and a text above the button (below it when the space above is small).
function ui.tip(anchor, title, body, stageW)
  local maxW = 16
  local pad, size, line = 0.7, 0.85, 0.85 * 1.4
  local lines = gfx.wrap(body, 'body', size, maxW - 2 * pad)
  local width = gfx.textWidth(title:upper(), 'bold', 0.75, 0.08)
  for _, l in ipairs(lines) do width = math.max(width, gfx.textWidth(l, 'body', size)) end
  local w = math.min(maxW, width + 2 * pad)
  local h = 0.5 + 1.05 + #lines * line + 0.5
  local cx = anchor.x + anchor.w / 2
  local x = math.max(0.5, math.min(cx - w / 2, stageW - w - 0.5))
  local y = anchor.y - h - 0.625
  local below = y < 0.5
  if below then y = anchor.y + anchor.h + 0.625 end
  gfx.shadow(x, y, w, h, px(4), px(8), px(18), 0, 0.6)
  gfx.rect(x, y + px(1), w, h, px(4), C.paperEdge)
  gfx.gradientRect(x, y, w, h, px(4), C.paperHi, C.paperLo)
  -- The arrow points to the button.
  local ay = below and y or y + h
  gfx.setColor(below and C.paperHi or C.paperLo)
  lg.polygon('fill', cx - 0.42, ay, cx + 0.42, ay, cx, ay + (below and -0.42 or 0.42))
  gfx.text(title:upper(), 'bold', 0.75, x + pad, y + 0.5, { color = C.paperHead, tracking = 0.08, line = 1.05 })
  gfx.lines(lines, 'body', size, x + pad, y + 0.5 + 1.05, line, { color = C.paperInk })
end

--[[
  The pips of the levels of an upgrade. Returns the width. `fresh` is the level that the player bought a moment ago, and
  `since` the time since the purchase: its pip grows into view.
]]
function ui.pips(x, cy, level, max, size, gap, fresh, since)
  for i = 1, max do
    local px_ = x + (i - 1) * (size + gap) + size / 2
    local on = i <= level
    local scale = 1
    if on and fresh == i and since then
      local t = (since - 0.15) / 0.5
      if t < 0 then scale = 0 elseif t < 0.6 then scale = 1.7 * t / 0.6 elseif t < 1 then scale = 1.7 - 0.7 * (t - 0.6) / 0.4 end
    end
    if on then
      if scale > 0.01 then gfx.circle(px_, cy, size / 2 * scale, C.accent) end
    else
      gfx.ring(px_, cy, size / 2 - px(0.5), px(1), C.dim)
    end
  end
  return max * size + (max - 1) * gap
end

-- Sparks that fly out from a point. `since` is the time since the start, `delay` the time before the sparks.
function ui.burst(cx, cy, since, count, delay)
  local t = (since - delay) / 1.1
  if t <= 0 or t >= 1 then return end
  local e = gfx.ease(t)
  for i = 0, count - 1 do
    local angle = i / count * 2 * math.pi
    local reach = (3 + (i % 3) * 1.25) * e
    local small = i % 2 == 0
    local r = (small and 0.15 or 0.25) * (1 - 0.7 * e)
    gfx.circle(cx + math.sin(angle) * reach, cy - math.cos(angle) * reach, r, small and C.light or C.accent, 1 - e)
  end
end

-- A row of a tally that rises into view. `since` is the time since the start of the row.
function ui.tallyRow(x, y, w, label, value, since, total)
  local t = since / 0.3
  if t <= 0 then return end
  local size, face = total and 1.15 or 1, total and 'semibold' or 'body'
  gfx.withAlpha(math.min(1, t), function()
    local rise = 0.4 * (1 - gfx.ease(t))
    local line = size * 1.5
    gfx.text(label, face, size, x, y + rise, { color = total and C.text or C.dim, line = line })
    gfx.text(ui.counted(0, value, since), 'semibold', size, x, y + rise, { color = C.gold, align = 'right', width = w, line = line })
  end)
end

-- The question before an action that ends a run. The web game uses the dialog of the browser for it.
function ui.dialogLayout(stage)
  local w, h = 20, 7.4
  local x, y = (stage.w - w) / 2, (stage.h - h) / 2
  return {
    x = x, y = y, w = w, h = h,
    cancel = { x = x + w - 1.25 - 4.2 - 0.7 - 5.2, y = y + h - 1.25 - 2.25, w = 5.2, h = 2.25 },
    ok = { x = x + w - 1.25 - 4.2, y = y + h - 1.25 - 2.25, w = 4.2, h = 2.25 },
  }
end

function ui.dialog(stage, question, pointer)
  local d = ui.dialogLayout(stage)
  gfx.rect(0, 0, stage.w, stage.h, 0, C.black, 0.45)
  gfx.shadow(d.x, d.y, d.w, d.h, px(10), px(10), px(40), 0, 0.6)
  gfx.rect(d.x, d.y, d.w, d.h, px(10), C.panel)
  gfx.outline(d.x, d.y, d.w, d.h, px(10), px(1), C.line)
  local lines = gfx.wrap(question, 'body', 1.05, d.w - 2.5)
  gfx.lines(lines, 'body', 1.05, d.x + 1.25, d.y + 1.25, 1.6, {})
  ui.key(d.cancel, 'Cancel', 'key', ui.state(pointer, 'cancel'))
  ui.key(d.ok, 'OK', 'key', ui.state(pointer, 'ok'))
end

return ui
