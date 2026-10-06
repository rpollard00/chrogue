-- Drawing helpers. Each position and each size is in stage units. gfx.u is the number of pixels in one unit.
-- The surfaces are the objects of DESIGN.md: raised (plaque, card, key), recessed (well), and the medal.
local theme = require('theme')
local utf8 = require('utf8')
local lg = love.graphics
local C = theme.color

local gfx = { u = 16, alpha = 1 }

local function px(n) return n / 16 end
gfx.px = px

function gfx.setColor(c, a) lg.setColor(c[1], c[2], c[3], (c[4] or 1) * (a or 1) * gfx.alpha) end

-- Runs a function with all its drawing at a part of the opacity.
function gfx.withAlpha(a, draw)
  local before = gfx.alpha
  gfx.alpha = before * a
  draw()
  gfx.alpha = before
end

function gfx.ease(t) t = math.max(0, math.min(1, t)) return 1 - (1 - t) ^ 3 end

-- Shapes

function gfx.rect(x, y, w, h, radius, color, a)
  gfx.setColor(color, a)
  lg.rectangle('fill', x, y, w, h, radius or 0, radius or 0, 12)
end

function gfx.outline(x, y, w, h, radius, width, color, a)
  gfx.setColor(color, a)
  lg.setLineWidth(width)
  lg.rectangle('line', x + width / 2, y + width / 2, w - width, h - width, math.max(0, (radius or 0) - width / 2), nil, 12)
end

function gfx.circle(cx, cy, r, color, a)
  gfx.setColor(color, a)
  lg.circle('fill', cx, cy, r, 48)
end

function gfx.ring(cx, cy, r, width, color, a)
  gfx.setColor(color, a)
  lg.setLineWidth(width)
  lg.circle('line', cx, cy, r, 48)
end

-- A ring of `count` arcs with gaps between them. Each arc is `sweep` radians long. The first arc has its center at the top.
function gfx.arcs(cx, cy, r, width, count, sweep, color, a)
  gfx.setColor(color, a)
  lg.setLineWidth(width)
  for i = 0, count - 1 do
    local center = -math.pi / 2 + i * 2 * math.pi / count
    lg.arc('line', 'open', cx, cy, r, center - sweep / 2, center + sweep / 2, 8)
  end
end

local quad
local function vertex(x, y, c) return { x, y, 0, 0, c[1], c[2], c[3], c[4] or 1 } end

-- Fills a shape with a gradient from the top to the bottom. `shape` draws the shape. With `diagonal`, the gradient
-- goes from the top left corner to the bottom right corner, as the 160 degree gradient of a raised surface.
function gfx.gradient(shape, x, y, w, h, top, bottom, diagonal)
  quad = quad or lg.newMesh(4, 'fan', 'stream')
  local side = diagonal and theme.mix(top, 0.5, bottom) or nil
  quad:setVertices({ vertex(x, y, top), vertex(x + w, y, side or top), vertex(x + w, y + h, bottom), vertex(x, y + h, side or bottom) })
  lg.stencil(shape, 'replace', 1)
  lg.setStencilTest('equal', 1)
  lg.setColor(1, 1, 1, gfx.alpha)
  lg.draw(quad)
  lg.setStencilTest()
end

function gfx.gradientRect(x, y, w, h, radius, top, bottom, diagonal)
  gfx.gradient(function() lg.rectangle('fill', x, y, w, h, radius, radius, 12) end, x, y, w, h, top, bottom, diagonal)
end

-- The disks of gfx.radial: one mesh of radius 1 for each pair of colors. A color is a stable table (theme.lua).
local disks = setmetatable({}, { __mode = 'k' })

-- Fills a circle with a radial gradient. The center of the gradient is above the center of the circle, as a light from above.
function gfx.radial(cx, cy, r, inner, outer)
  local byOuter = disks[inner]
  if not byOuter then byOuter = setmetatable({}, { __mode = 'k' }); disks[inner] = byOuter end
  local disk = byOuter[outer]
  if not disk then
    local vertices = { vertex(0, -0.4, inner) }
    for i = 0, 40 do
      local a = i / 40 * 2 * math.pi
      vertices[#vertices + 1] = vertex(math.cos(a), math.sin(a), outer)
    end
    disk = lg.newMesh(vertices, 'fan', 'static')
    byOuter[outer] = disk
  end
  lg.setColor(1, 1, 1, gfx.alpha)
  lg.draw(disk, cx, cy, 0, r, r)
end

-- A soft shadow: rectangles that become larger and more transparent. `alpha` is the opacity in the center.
function gfx.shadow(x, y, w, h, radius, dy, blur, spread, alpha)
  local steps = math.max(6, math.ceil(blur * gfx.u / 1.5))
  x, y, w, h = x - spread, y - spread + dy, w + 2 * spread, h + 2 * spread
  -- Each layer covers the layers before it, thus the opacity of one layer is small.
  local layer = 1 - (1 - alpha) ^ (1 / steps)
  for i = 1, steps do
    local grow = blur * (i / steps - 0.5)
    lg.setColor(0, 0, 0, layer * gfx.alpha)
    lg.rectangle('fill', x - grow, y - grow, w + 2 * grow, h + 2 * grow, math.max(0, radius + grow), nil, 12)
  end
end

-- A raised surface: the plaque and the card. `top` is the color of the line at the top edge.
function gfx.raised(x, y, w, h, radius, opts)
  opts = opts or {}
  gfx.shadow(x, y, w, h, radius, px(12), px(14), -px(8), 0.75)
  gfx.rect(x, y + px(3), w, h, radius, C.edge)
  gfx.gradientRect(x, y, w, h, radius, C.raisedHi, C.raisedLo, true)
  if opts.tint then
    gfx.gradientRect(x, y, w, h * 0.6, radius, theme.alpha(opts.tint, 0.26), theme.alpha(opts.tint, 0))
  end
  gfx.outline(x, y, w, h, radius, px(1), opts.rim or C.rim)
  gfx.topLight(x, y, w, h, radius, opts.top and px(2) or px(1), opts.top or C.shine)
end

-- The light on the top edge of a raised surface. It follows the round corners.
function gfx.topLight(x, y, w, h, radius, width, color)
  lg.stencil(function() lg.rectangle('fill', x, y, w, h, radius, radius, 12) end, 'replace', 1)
  lg.stencil(function() lg.rectangle('fill', x, y + width, w, h, radius, radius, 12) end, 'replace', 0, true)
  lg.setStencilTest('equal', 1)
  gfx.rect(x, y, w, radius + width, 0, color)
  lg.setStencilTest()
end

-- A recessed surface: the well. `lined` gives the brown lining, thus a black piece is visible on it.
function gfx.well(x, y, w, h, radius, lined)
  local shape = function() lg.rectangle('fill', x, y, w, h, radius, radius, 12) end
  if lined then gfx.gradient(shape, x, y, w, h, C.liningLo, C.lining) else gfx.gradient(shape, x, y, w, h, C.wellHi, C.wellLo) end
  -- The inner shadow at the top.
  gfx.gradient(shape, x, y, w, px(6), theme.alpha(C.black, 0.55), theme.alpha(C.black, 0))
  gfx.setColor(C.shineSoft)
  lg.setLineWidth(px(1))
  lg.line(x + radius * 0.6, y + h - px(0.5), x + w - radius * 0.6, y + h - px(0.5))
end

-- A shelf: a recessed area that holds a row of cards. Its shadow is deeper than the shadow of a well.
function gfx.shelf(x, y, w, h, radius)
  local shape = function() lg.rectangle('fill', x, y, w, h, radius, radius, 12) end
  gfx.gradient(shape, x, y, w, h, C.shelfHi, C.shelfLo)
  gfx.gradient(shape, x, y, w, px(10), theme.alpha(C.black, 0.7), theme.alpha(C.black, 0))
  gfx.setColor(C.shineSoft)
  lg.setLineWidth(px(1))
  lg.line(x + radius * 0.6, y + h - px(0.5), x + w - radius * 0.6, y + h - px(0.5))
end

--[[
  A key. `kind` is 'key', 'quiet' or 'primary'. A key goes down when the player presses it.
  A key that the player cannot use (state.disabled) stays down and has no light.
  Returns the distance that the face went down, and the color of the label.
]]
function gfx.key(x, y, w, h, kind, state)
  local primary = kind == 'primary'
  local radius = primary and px(8) or px(6)
  local depth = primary and px(4) or (kind == 'quiet' and px(2) or px(3))
  local hi, lo, edge = C.keyHi, C.keyLo, C.keyEdge
  local ink = primary and C.amberInk or (kind == 'quiet' and C.dim or C.text)
  local disabled = state.disabled
  if primary then hi, lo, edge = C.amberHi, C.amberLo, C.amberEdge
  elseif kind == 'quiet' then hi, lo = C.keyOffHi, C.keyOffLo
  elseif state.hover and not disabled then hi, lo = C.keyHoverHi, C.keyHoverLo end
  local down = state.pressed and not disabled and (depth - px(1)) or 0
  if disabled then
    down = depth - px(1)
    if primary then hi, lo, edge, ink = C.amberOffHi, C.amberOffLo, C.amberOffEdge, C.amberOffInk
    else hi, lo, ink = C.keyOffHi, C.keyOffLo, C.dim end
  end
  if down == 0 and kind ~= 'quiet' then gfx.shadow(x, y, w, h, radius, primary and px(7) or px(4), px(7), 0, 0.5) end
  gfx.rect(x, y + depth, w, h, radius, edge)
  gfx.gradientRect(x, y + down, w, h, radius, hi, lo)
  if primary and state.hover and not disabled then gfx.rect(x, y + down, w, h, radius, C.white, 0.08) end
  gfx.setColor((primary and not disabled) and C.shineHard or ((kind == 'quiet' or disabled) and C.shineSoft or C.shine))
  lg.setLineWidth(px(1))
  lg.line(x + radius * 0.7, y + down + px(0.5), x + w - radius * 0.7, y + down + px(0.5))
  return down, ink
end

local gray
-- Draws with less color and less opacity. `amount` is from 0 to 1.
function gfx.muted(amount, alpha, draw)
  gray = gray or lg.newShader([[
    extern float amount;
    vec4 effect(vec4 color, Image tex, vec2 uv, vec2 sc) {
      vec4 c = Texel(tex, uv) * color;
      float l = dot(c.rgb, vec3(0.299, 0.587, 0.114));
      return vec4(mix(c.rgb, vec3(l), amount), c.a);
    }
  ]])
  local before = lg.getShader()
  gray:send('amount', amount)
  lg.setShader(gray)
  gfx.withAlpha(alpha, draw)
  lg.setShader(before)
end

-- Draws a function with a scale around a point.
function gfx.scaled(cx, cy, sx, sy, draw)
  lg.push()
  lg.translate(cx, cy)
  lg.scale(sx, sy or sx)
  lg.translate(-cx, -cy)
  draw()
  lg.pop()
end

-- A medal: a disk that is pressed into its surface. `flair` is the color of its rim and of its picture.
function gfx.medal(cx, cy, size, flair, lined)
  local r = size / 2
  gfx.circle(cx, cy + px(1), r + px(2), C.black, 0.5)
  gfx.circle(cx, cy, r + px(1.5), theme.mix(flair, 0.7, C.black))
  if lined then gfx.radial(cx, cy, r, C.liningHi, C.liningLo) else gfx.radial(cx, cy, r, C.medalHi, C.medalLo) end
  gfx.ring(cx, cy, r - px(0.75), px(1.5), C.black, 0.28)
end

-- Text

local function fontOf(face, size) return theme.font(face, size * gfx.u) end
gfx.font = fontOf

-- The width of a text in units. `tracking` is the letter spacing in em.
function gfx.textWidth(text, face, size, tracking)
  local width = fontOf(face, size):getWidth(text) / gfx.u
  if tracking then width = width + utf8.len(text) * tracking * size end
  return width
end

local function snap(v) return math.floor(v * gfx.u + 0.5) / gfx.u end

--[[
  Draws one line of text. y is the top of the line box, and opts.line is the height of the line box (the default is 1.2 em).
  opts: color, alpha, align ('left', 'center', 'right') in opts.width, tracking in em.
  Returns the width of the text.
]]
function gfx.text(text, face, size, x, y, opts)
  opts = opts or {}
  local font = fontOf(face, size)
  local s = 1 / gfx.u
  local width = gfx.textWidth(text, face, size, opts.tracking)
  if opts.align == 'center' then x = x + ((opts.width or 0) - width) / 2
  elseif opts.align == 'right' then x = x + (opts.width or 0) - width end
  y = y + ((opts.line or size * 1.2) - font:getHeight() * s) / 2
  if opts.shadow then
    gfx.setColor(opts.shadow, opts.alpha)
    lg.print(text, font, snap(x), snap(y + (opts.shadowY or px(2))), 0, s, s)
  end
  gfx.setColor(opts.color or C.text, opts.alpha)
  if opts.tracking then
    local at, step = x, opts.tracking * size
    for _, code in utf8.codes(text) do
      local ch = utf8.char(code)
      lg.print(ch, font, snap(at), snap(y), 0, s, s)
      at = at + font:getWidth(ch) * s + step
    end
  else
    lg.print(text, font, snap(x), snap(y), 0, s, s)
  end
  return width
end

-- The lines of each wrapped text, by text, face, size, width, balance, and the size of the unit.
local wrapped, wrapCount = {}, 0

-- Breaks a text into lines of a width. With `balance`, the lines get almost the same length, as text-wrap: balance.
-- The result is in a cache. Do not change the list.
function gfx.wrap(text, face, size, width, balance)
  local key = table.concat({ text, face, size, width, balance and 1 or 0, gfx.u }, '\0')
  local lines = wrapped[key]
  if lines then return lines end
  if wrapCount > 4000 then wrapped, wrapCount = {}, 0 end
  lines = gfx.wrapNow(text, face, size, width, balance)
  wrapped[key], wrapCount = lines, wrapCount + 1
  return lines
end

function gfx.wrapNow(text, face, size, width, balance)
  local font = fontOf(face, size)
  local _, lines = font:getWrap(text, width * gfx.u)
  if balance and #lines > 1 then
    for step = 1, 12 do
      local _, tighter = font:getWrap(text, width * gfx.u * (1 - step * 0.04))
      if #tighter > #lines then break end
      lines = tighter
    end
  end
  return lines
end

-- Draws lines of text. Returns the height of the block.
function gfx.lines(lines, face, size, x, y, lineHeight, opts)
  for i, line in ipairs(lines) do
    opts.line = lineHeight
    gfx.text(line, face, size, x, y + (i - 1) * lineHeight, opts)
  end
  return #lines * lineHeight
end

-- Pieces

local GLYPH = { k = '♚', q = '♛', r = '♜', b = '♝', n = '♞', p = '♟' }

-- Draws a piece with its center at a point. `size` is the font size in units. The glyph is solid for each color,
-- and it has an outline, thus a piece is readable on each square and on the lining.
function gfx.piece(kind, color, cx, cy, size, alpha, scale)
  if scale and scale ~= 1 then
    -- The scale of an animation is a transform. A font for each scaled size would load the typeface again in each frame.
    lg.push()
    lg.translate(cx, cy)
    lg.scale(scale)
    gfx.piece(kind, color, 0, 0, size, alpha)
    lg.pop()
    return
  end
  local font = fontOf('piece', size)
  local glyph, s = GLYPH[kind], 1 / gfx.u
  local x = cx - font:getWidth(glyph) * s / 2
  local y = cy - (font:getAscent() - font:getDescent()) * s / 2
  local fill, stroke = C.pieceW, C.pieceWStroke
  -- At 16 pixels for each unit, the outline is 1.5 pixels for a white piece and 1 pixel for a black piece. Half of it
  -- is outside the glyph.
  local out = 0.75 * gfx.u / 16
  if color == 'b' then fill, stroke, out = C.pieceB, C.pieceBStroke, 0.5 * gfx.u / 16 end
  out = math.max(out, 0.6) * s
  gfx.setColor(stroke, alpha)
  for i = 0, 7 do
    local a = i * math.pi / 4
    lg.print(glyph, font, x + math.cos(a) * out, y + math.sin(a) * out, 0, s, s)
  end
  gfx.setColor(fill, alpha)
  lg.print(glyph, font, x, y, 0, s, s)
end

return gfx
