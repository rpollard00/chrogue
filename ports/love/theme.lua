-- The colors and the typefaces.
local theme = {}

--[[
  A color is a table { r, g, b, a }. The functions below give the same table for the same arguments, thus a drawing in
  each frame does not parse or mix a color again, and a mesh can keep a color table as its key. Do not change a color table.
]]
local parsed = {}
local function hex(text, alpha)
  local key = text .. (alpha and ('/' .. alpha) or '')
  local c = parsed[key]
  if not c then
    local r, g, b = text:match('^#(%x%x)(%x%x)(%x%x)$')
    c = { tonumber(r, 16) / 255, tonumber(g, 16) / 255, tonumber(b, 16) / 255, alpha or 1 }
    parsed[key] = c
  end
  return c
end
local function rgba(r, g, b, a) return { r / 255, g / 255, b / 255, a } end

theme.hex = hex

-- A cache with weak keys: an entry goes when no other table has the color.
local function weak() return setmetatable({}, { __mode = 'k' }) end
local mixes, alphas = weak(), weak()

-- color-mix(in srgb, a p%, b), as a new table. For a mix that changes in each frame (an animation).
function theme.mixNow(a, p, b)
  local q = 1 - p
  return { a[1] * p + b[1] * q, a[2] * p + b[2] * q, a[3] * p + b[3] * q, (a[4] or 1) * p + (b[4] or 1) * q }
end

-- color-mix(in srgb, a p%, b), from the cache. For a mix with a set `p`.
function theme.mix(a, p, b)
  local byP = mixes[a]
  if not byP then byP = {}; mixes[a] = byP end
  local byB = byP[p]
  if not byB then byB = weak(); byP[p] = byB end
  local c = byB[b]
  if not c then
    local q = 1 - p
    c = { a[1] * p + b[1] * q, a[2] * p + b[2] * q, a[3] * p + b[3] * q, (a[4] or 1) * p + (b[4] or 1) * q }
    byB[b] = c
  end
  return c
end

function theme.alpha(c, a)
  local byA = alphas[c]
  if not byA then byA = {}; alphas[c] = byA end
  local out = byA[a]
  if not out then
    out = { c[1], c[2], c[3], (c[4] or 1) * a }
    byA[a] = out
  end
  return out
end

theme.color = {
  bg = hex('#15171c'), panel = hex('#1f232b'), line = hex('#333a47'), text = hex('#e8e6e1'), dim = hex('#9aa0ab'),
  accent = hex('#d9a441'), gold = hex('#e6c34a'), danger = hex('#c8564b'), light = hex('#d8cbb0'), dark = hex('#8a6f52'),
  unit = hex('#86aede'), relic = hex('#5fc296'), upgrade = hex('#b698e6'),
  raisedHi = hex('#2d323d'), raisedLo = hex('#1b1e25'), rim = hex('#14161b'), edge = hex('#0b0c0f'),
  keyHi = hex('#3a4250'), keyLo = hex('#2a303b'), keyHoverHi = hex('#444d5d'), keyHoverLo = hex('#303743'),
  keyOffHi = hex('#2a2f39'), keyOffLo = hex('#242830'), keyEdge = hex('#0e1014'),
  amberHi = hex('#efc060'), amberLo = hex('#cf9632'), amberEdge = hex('#6b4a12'), amberInk = hex('#1a1408'),
  amberOffHi = hex('#6b5628'), amberOffLo = hex('#59471f'), amberOffEdge = hex('#2a2009'), amberOffInk = hex('#d8ccae'),
  wellHi = hex('#121419'), wellLo = hex('#1a1d24'), shelfHi = hex('#101216'), shelfLo = hex('#171a20'),
  paperHi = hex('#efe4c8'), paperLo = hex('#e0d2ad'), paperInk = hex('#2b2417'), paperHead = hex('#6b5320'), paperEdge = hex('#8f7f58'),
  infoHi = hex('#f1dfae'), infoLo = hex('#b89a56'), infoInk = hex('#2a2110'),
  liningHi = hex('#7a634b'), lining = hex('#675440'), liningLo = hex('#54432f'), liningInk = hex('#e2d8c0'),
  medalRim = hex('#c9c4b8'), lamp = hex('#f2c468'), lampCore = hex('#ffe7a8'),
  bossHi = hex('#e27b6f'), bossInk = hex('#1c0a08'), bossEdge = hex('#5b1d16'),
  shine = rgba(255, 255, 255, 0.14), shineSoft = rgba(255, 255, 255, 0.08), shineHard = rgba(255, 255, 255, 0.5),
  shade = rgba(0, 0, 0, 0.75), shadeSoft = rgba(0, 0, 0, 0.5),
  black = hex('#000000'), white = hex('#ffffff'),
  -- The board and the pieces.
  frame = hex('#3b2f24'),
  pieceW = hex('#fbf8f0'), pieceWStroke = hex('#1c1a17'), pieceB = hex('#17161a'), pieceBStroke = hex('#8d8a84'),
  markLast = rgba(217, 164, 65, 0.4), markSelected = rgba(90, 160, 90, 0.6), markCheck = rgba(200, 50, 40, 0.65),
  markAlarm = rgba(255, 70, 50, 0.95), markTarget = rgba(30, 30, 30, 0.4), markCapture = rgba(30, 30, 30, 0.45),
  scoutSelected = rgba(200, 86, 75, 0.55), scoutTarget = rgba(160, 30, 22, 0.6), scoutCapture = rgba(160, 30, 22, 0.75),
  coord = rgba(20, 20, 20, 0.6),
  medalHi = hex('#2e343f'), medalLo = hex('#14161b'), chipHi = hex('#2c323d'), chipLo = hex('#22262f'),
  flash = rgba(217, 164, 65, 0.6),
}

-- The display typeface is Fira Sans Condensed ExtraBold. Body text uses Fira Sans. The pieces are the chess glyphs of
-- DejaVu Sans.
local FILES = {
  display = 'fonts/FiraSansCondensed-ExtraBold.ttf',
  body = 'fonts/FiraSans-Regular.ttf',
  semibold = 'fonts/FiraSans-SemiBold.ttf',
  bold = 'fonts/FiraSans-Bold.ttf',
  piece = 'fonts/DejaVuSans.ttf',
}

local fonts, data = {}, {}
-- The number of fonts that the game made, for the dump of the test script.
theme.fontsMade = 0
-- The pixels of a canvas for each unit of the window (shaders.lua). A font has its glyphs at this density.
theme.density = 1

-- Returns the font of a face for a size in pixels. A change of the window size makes new fonts.
function theme.font(face, pixels)
  pixels = math.max(6, math.floor(pixels + 0.5))
  local key = face .. pixels
  local font = fonts[key]
  if not font then
    -- The file of each typeface is read one time.
    data[face] = data[face] or love.filesystem.newFileData(FILES[face])
    font = love.graphics.newFont(data[face], pixels, 'light', theme.density)
    fonts[key] = font
    theme.fontsMade = theme.fontsMade + 1
  end
  return font
end

function theme.dropFonts() fonts = {} end

return theme
