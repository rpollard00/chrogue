-- The three shaders of the experiment, and the canvases that they need.
-- 1. background: the surface behind the layout. The swirl is the default. The debug menu selects a different one.
-- 2. post: the full scene goes to a canvas, and the canvas goes to the window through this shader. It gives the look
--    of a screen. The tube screen (CRT) is the default. The debug menu selects a different one.
-- 3. foil: a sheen on the relic medals and on the relic card. It moves with the time and with the pointer.
local gfx = require('gfx')
local theme = require('theme')
local lg = love.graphics

local shaders = {}

-- F1 changes the mode.
shaders.MODES = {
  { label = 'Effects: all on', name = 'All on', background = true, foil = true, post = true },
  { label = 'Effects: post pass off', name = 'Post pass off', background = true, foil = true, post = false },
  { label = 'Effects: all off', name = 'All off', background = false, foil = false, post = false },
}
shaders.mode = 1

-- OpenGL ES (a browser, a phone) gives a pixel shader floats of medium precision if the shader does not ask for more:
-- about 3 digits. The noise of the background and the screen positions need more. LÖVE declares `effect` with medium
-- precision, thus each `effect` here has the same parameters, and it reads the position of the pixel from
-- love_PixelCoord, not from its last parameter.
local PRECISION = [[
#if defined(GL_ES) && defined(GL_FRAGMENT_PRECISION_HIGH)
precision highp float;
#endif
]]

local SWIRL = PRECISION .. [[
extern float time;
extern vec2 resolution;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
  float v = 0.0, a = 0.5;
  for (int i = 0; i < 5; i++) {
    v += a * noise(p);
    p = p * 2.03 + vec2(17.0, 9.0);
    a *= 0.5;
  }
  return v;
}

vec4 effect(mediump vec4 color, Image tex, mediump vec2 uv, mediump vec2 pixel) {
  vec2 sc = love_PixelCoord;
  vec2 p = (sc - 0.5 * resolution) / resolution.y;
  float t = time * 0.045;
  // The swirl: a turn that is larger near the center.
  float r = length(p);
  float angle = 1.4 * exp(-r * 1.3) + t * 0.6;
  p = mat2(cos(angle), -sin(angle), sin(angle), cos(angle)) * p;
  // The domain warp: the noise moves the position of the next noise, two times.
  vec2 q = vec2(fbm(p * 1.7 + vec2(0.0, t)), fbm(p * 1.7 + vec2(5.2, -t * 0.8)));
  vec2 w = vec2(fbm(p * 1.7 + 3.0 * q + vec2(1.7, 9.2) + t * 0.7), fbm(p * 1.7 + 3.0 * q + vec2(8.3, 2.8) - t * 0.6));
  float f = fbm(p * 1.7 + 3.5 * w);
  // The dark colors of the game: the background, the panel, and a small part of amber and of the relic green.
  vec3 deep = vec3(0.040, 0.044, 0.058);
  vec3 panel = vec3(0.185, 0.205, 0.255);
  vec3 col = mix(deep, panel, smoothstep(0.15, 0.85, f));
  col = mix(col, vec3(0.30, 0.215, 0.085), smoothstep(0.55, 1.0, w.y) * 0.55);
  col = mix(col, vec3(0.085, 0.215, 0.180), smoothstep(0.50, 0.95, q.x) * smoothstep(0.3, 0.7, f) * 0.5);
  col = mix(col, vec3(0.26, 0.10, 0.09), smoothstep(0.62, 1.0, w.x) * 0.35);
  col *= 1.0 - 0.45 * smoothstep(0.3, 1.1, r);
  col += (hash(sc + time) - 0.5) / 255.0;
  return vec4(col, 1.0);
}
]]

--[[
  The other backgrounds are experiments for the look of the game. Each one gives `scene`: the color at `p`, a point in
  stage units from the center of the window. `u` is the pixels of one unit.
]]
local SCENE_HEAD = PRECISION .. [[
extern float time;
extern vec2 resolution;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
  float v = 0.0, a = 0.5;
  for (int i = 0; i < 5; i++) {
    v += a * noise(p);
    p = p * 2.03 + vec2(17.0, 9.0);
    a *= 0.5;
  }
  return v;
}

// One lamp above the center of the table. The light is wide and soft, and it changes slowly.
float lamp(vec2 p) {
  float r = length((p - vec2(0.0, -5.0)) * vec2(0.60, 1.0));
  return exp(-r * r / 620.0) * (1.0 + 0.025 * sin(time * 0.35));
}

// The dark surface of the line drawings, with dark corners.
vec3 slate(vec2 p, vec3 ink, float a) {
  return mix(vec3(0.066, 0.072, 0.088), ink, a) * (1.0 - 0.38 * smoothstep(20.0, 46.0, length(p * vec2(0.8, 1.0))));
}
]]

local SCENE_MAIN = [[
vec4 effect(mediump vec4 color, Image tex, mediump vec2 uv, mediump vec2 pixel) {
  vec2 sc = love_PixelCoord;
  float u = min(resolution.x / 80.0, resolution.y / 45.0);
  vec3 col = scene((sc - 0.5 * resolution) / u, u);
  col += (hash(sc) - 0.5) * 2.0 / 255.0;
  return vec4(col, 1.0);
}
]]

-- The hatch of a printed chess diagram: lines at 45 degrees on the dark squares.
local HATCH = [[
vec3 scene(vec2 p, float u) {
  float S = 7.5;
  vec2 g = p / S;
  vec2 cell = floor(g);
  float dark = mod(cell.x + cell.y, 2.0);
  float period = 0.42;
  float c = abs(fract((p.x + p.y) * 0.7071 / period) - 0.5) * period;
  float aa = 0.75 / u;
  float hatch = 1.0 - smoothstep(0.055 - aa, 0.055 + aa, c);
  // The frame of each square: one thin line.
  vec2 e = abs(fract(g) - 0.5) * S;
  float frame = 1.0 - smoothstep(0.03 - aa, 0.03 + aa, S * 0.5 - max(e.x, e.y));
  // One square at a time becomes a small amount brighter, and then goes back.
  float wake = smoothstep(0.90, 1.0, sin(time * 0.22 + hash(cell) * 6.2831853));
  return slate(p, vec3(0.150, 0.166, 0.205), dark * hatch * (0.55 + 0.45 * wake) + frame * 0.35);
}
]]

--[[
  A closed knight's tour: one line through the 64 squares of a board. There is one figure at each side of the center,
  and the left figure is the mirror of the right figure. A light goes along the line, one move of the knight at a time.
]]
local TOUR = [[
extern vec2 path[65];

float segment(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a, ba = b - a;
  return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

float tour(vec2 p, float u) {
  // The distance between two squares. The figure has the height of the stage, thus only one row of figures shows.
  vec2 L = vec2(5.0, 6.0);
  float side = mod(floor(p.x / 40.0), 2.0);
  vec2 l = vec2(mod(p.x, 40.0) - 20.0, p.y);
  if (side > 0.5) l.x = -l.x;
  float aa = 0.75 / u;
  float head = mod(time * 1.1, 64.0);
  float a = 0.0;
  for (int i = 0; i < 64; i++) {
    float d = segment(l, (path[i] - 3.5) * L, (path[i + 1] - 3.5) * L);
    float lit = 0.42 + 0.58 * exp(-mod(head - float(i), 64.0) * 0.22);
    a = max(a, (1.0 - smoothstep(0.055 - aa, 0.055 + aa, d)) * lit);
  }
  return a;
}
]]

-- The nap of green baize: short fibers in each direction, and a soft mottle where a hand went across it.
local FELT = [[
vec3 felt(vec2 p) {
  float fiber = noise(p * vec2(26.0, 7.0)) + noise(p.yx * vec2(26.0, 7.0) + 31.0) + noise((p.x + p.y) * vec2(14.0, 0.0) + (p.x - p.y) * vec2(0.0, 5.0));
  float mottle = fbm(p * 0.16) - 0.5;
  vec3 col = mix(vec3(0.018, 0.050, 0.040), vec3(0.060, 0.170, 0.125), clamp(lamp(p) * 0.95 + mottle * 0.22, 0.0, 1.0));
  return col * (1.0 + (fiber / 3.0 - 0.5) * 0.34);
}
]]

-- A table top of walnut: wide boards along the screen. Each board has its own grain and tone.
local WALNUT = [[
vec3 walnut(vec2 p) {
  float H = 12.5;
  float row = floor(p.y / H + 0.5);
  float inBoard = (fract(p.y / H + 0.5) - 0.5) * H;
  vec2 g = vec2(p.x + row * 37.0, p.y + row * 5.3);
  float rings = 0.5 + 0.5 * sin((g.y * 1.9 + fbm(g * vec2(0.035, 0.22)) * 22.0) * 1.25);
  float grain = rings * 0.45 + fbm(g * vec2(0.10, 5.5)) * 0.40 + noise(g * vec2(1.6, 42.0)) * 0.15;
  float tone = 0.86 + 0.28 * hash(vec2(row, 3.0));
  vec3 col = mix(vec3(0.050, 0.030, 0.020), vec3(0.230, 0.132, 0.072), grain * tone * (0.22 + 0.78 * lamp(p)));
  return col * (0.45 + 0.55 * smoothstep(0.0, 0.09, H * 0.5 - abs(inBoard)));
}
]]

-- The top of a games table: leather with a pebble grain, and two gilt lines near the edge of the stage.
local LEATHER = [[
// The distance to the nearest point of a cell pattern.
float cells(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  float d = 1.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 o = vec2(float(x), float(y));
      vec2 c = o + vec2(hash(i + o), hash(i + o + 7.7)) - f;
      d = min(d, dot(c, c));
    }
  }
  return sqrt(d);
}

float box(vec2 p, vec2 size) {
  vec2 d = abs(p) - size;
  return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

vec3 scene(vec2 p, float u) {
  float aa = 0.75 / u;
  float h = cells(p * 3.4);
  // The light from above is on the top side of each pebble.
  float slope = cells((p + vec2(0.0, 0.05)) * 3.4) - cells((p - vec2(0.0, 0.05)) * 3.4);
  float mottle = fbm(p * 0.22) - 0.5;
  vec3 col = mix(vec3(0.045, 0.014, 0.014), vec3(0.205, 0.062, 0.052), clamp(lamp(p) * 0.9 + mottle * 0.25, 0.0, 1.0));
  col *= 0.90 + 0.22 * h + slope * 0.9;
  float outer = 1.0 - smoothstep(0.07 - aa, 0.07 + aa, abs(box(p, vec2(39.45, 21.95))));
  float inner = 1.0 - smoothstep(0.025 - aa, 0.025 + aa, abs(box(p, vec2(39.05, 21.55))));
  return mix(col, vec3(0.62, 0.47, 0.22) * (0.55 + 0.6 * h), max(outer, inner * 0.8) * 0.75);
}
]]

local function scene(...) return SCENE_HEAD .. table.concat({ ... }) .. SCENE_MAIN end

-- The backgrounds, in the sequence of the debug menu. `note` tells where the picture comes from.
shaders.BACKGROUNDS = {
  { id = 'swirl', label = 'Swirl', note = 'The default: slow marble', source = SWIRL },
  { id = 'hatch', label = 'Diagram hatch', note = 'The dark squares of a printed diagram', source = scene(HATCH) },
  { id = 'tour', label = "Knight's tour", note = 'One line through the 64 squares',
    source = scene(TOUR, 'vec3 scene(vec2 p, float u) { return slate(p, vec3(0.185, 0.205, 0.255), tour(p, u)); }\n') },
  { id = 'lamp', label = 'Lamp', note = 'No picture: one soft light',
    source = scene('vec3 scene(vec2 p, float u) { return mix(vec3(0.040, 0.044, 0.056), vec3(0.125, 0.138, 0.170), lamp(p)); }\n') },
  { id = 'felt', label = 'Baize', note = 'The green cloth of a games table',
    source = scene(FELT, 'vec3 scene(vec2 p, float u) { return felt(p); }\n') },
  { id = 'walnut', label = 'Walnut', note = 'The boards of a chess table',
    source = scene(WALNUT, 'vec3 scene(vec2 p, float u) { return walnut(p); }\n') },
  { id = 'leather', label = 'Leather', note = 'A leather top with gilt lines', source = scene(LEATHER) },
  { id = 'feltTour', label = 'Baize and tour', note = "The knight's tour as chalk on baize",
    source = scene(FELT, TOUR, 'vec3 scene(vec2 p, float u) { return mix(felt(p), vec3(0.30, 0.46, 0.38), tour(p, u) * 0.55); }\n') },
  { id = 'walnutTour', label = 'Walnut and tour', note = "The knight's tour as brass inlay in walnut",
    source = scene(WALNUT, TOUR, 'vec3 scene(vec2 p, float u) { return mix(walnut(p), vec3(0.62, 0.47, 0.22) * (0.45 + 0.55 * lamp(p)), tour(p, u) * 0.8); }\n') },
  -- The field is flat: it has no lamp and no dark corners. Thus it shows what a post pass does to the picture.
  { id = 'bluescreen', label = 'Blue screen', note = 'No picture: one flat blue',
    source = scene('vec3 scene(vec2 p, float u) { return vec3(0.150, 0.185, 0.620); }\n') },
}
shaders.background = 'swirl'

-- The squares of the knight's tour, as points of a board from 0 to 7. The last point is the first point again.
local TOUR_PATH = {
  { 0, 0 }, { 2, 1 }, { 4, 0 }, { 6, 1 }, { 7, 3 }, { 6, 5 }, { 7, 7 }, { 5, 6 },
  { 7, 5 }, { 6, 7 }, { 4, 6 }, { 2, 7 }, { 0, 6 }, { 1, 4 }, { 0, 2 }, { 1, 0 },
  { 3, 1 }, { 5, 0 }, { 7, 1 }, { 6, 3 }, { 4, 2 }, { 3, 0 }, { 1, 1 }, { 0, 3 },
  { 1, 5 }, { 0, 7 }, { 2, 6 }, { 0, 5 }, { 1, 7 }, { 3, 6 }, { 5, 7 }, { 7, 6 },
  { 6, 4 }, { 5, 2 }, { 6, 0 }, { 7, 2 }, { 5, 1 }, { 7, 0 }, { 6, 2 }, { 5, 4 },
  { 6, 6 }, { 7, 4 }, { 5, 5 }, { 4, 7 }, { 3, 5 }, { 4, 3 }, { 2, 2 }, { 3, 4 },
  { 1, 3 }, { 0, 1 }, { 2, 0 }, { 4, 1 }, { 5, 3 }, { 3, 2 }, { 4, 4 }, { 2, 3 },
  { 0, 4 }, { 1, 6 }, { 3, 7 }, { 2, 5 }, { 3, 3 }, { 4, 5 }, { 2, 4 }, { 1, 2 },
}
TOUR_PATH[#TOUR_PATH + 1] = TOUR_PATH[1]

local FOIL = PRECISION .. [[
extern float time;
extern vec2 pointer;
extern float aspect;
extern float strength;

vec3 rainbow(float h) { return clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0); }

vec4 effect(mediump vec4 color, Image tex, mediump vec2 uv, mediump vec2 pixel) {
  vec4 base = Texel(tex, uv);
  // The foil is only on the solid surface, not on its shadow.
  float mask = smoothstep(0.9, 1.0, base.a);
  vec2 p = vec2(uv.x * aspect, uv.y);
  vec2 away = p - vec2(pointer.x * aspect, pointer.y);
  // The bands of color. The pointer tilts the surface, thus the bands move with it.
  float tilt = dot(away, vec2(0.55, 0.85));
  float ripple = sin(p.y * 7.0 + time * 0.9 + sin(p.x * 5.0 - time * 0.6) * 1.6) * 0.07;
  vec3 holo = mix(vec3(0.6), rainbow(fract(p.x * 0.9 + p.y * 0.6 + tilt * 0.9 + ripple + time * 0.06)), 0.8);
  // The sheen: one bright line that goes across the surface, and a soft light at the pointer.
  float sweep = fract(time * 0.16) * 3.2 - 1.1;
  float across = (p.x * 0.75 + p.y * 0.45 - sweep - pointer.x * 0.3) * 5.0;
  float line = exp(-across * across);
  float near = exp(-dot(away, away) * 3.0);
  float lum = dot(base.rgb, vec3(0.299, 0.587, 0.114));
  // The colors are strong at the pointer and on the bright line, and weak on the other parts. Thus the text stays easy to read.
  vec3 added = holo * (0.045 + 0.13 * near + 0.16 * line + 0.10 * lum) + vec3(1.0, 0.96, 0.86) * (line * 0.10 + near * 0.04);
  vec3 col = base.rgb + added * strength * mask * base.a;
  return vec4(col, base.a) * color;
}
]]

local CRT = PRECISION .. [[
extern vec2 resolution;
// The pixels of the window for each unit. A scan line has the same height on each screen.
extern float scale;
extern float curve;

vec4 effect(mediump vec4 color, Image tex, mediump vec2 uv, mediump vec2 pixel) {
  vec2 sc = love_PixelCoord;
  // The curve of a tube screen: a point is moved away from the center, and more near the corners.
  vec2 c = uv - 0.5;
  vec2 w = 0.5 + c * (1.0 + curve * dot(c, c)) / (1.0 + curve * 0.25);
  c = w - 0.5;
  // The red and the blue come from positions that are a small distance apart. The distance is 0 at the center.
  vec2 apart = c * dot(c, c) * 0.006;
  vec3 col = vec3(Texel(tex, w + apart).r, Texel(tex, w).g, Texel(tex, w - apart).b);
  // The scan lines are light, thus the text stays easy to read.
  float scan = 0.5 + 0.5 * sin(sc.y / scale * 6.2831853 / 3.0);
  col *= 1.025 - 0.055 * scan;
  col *= 1.0 - 0.32 * smoothstep(0.38, 0.86, length(c));
  vec2 inside = smoothstep(vec2(0.0), vec2(1.5) / resolution, w) * smoothstep(vec2(0.0), vec2(1.5) / resolution, 1.0 - w);
  return vec4(col * inside.x * inside.y, 1.0);
}
]]

--[[
  An old rear projection set: three tubes behind a flat screen. The screen is bright in a wide zone at its middle, and
  the color changes with the light: toward cyan in the zone, toward violet at the edges. Each term is small, thus the
  text stays easy to read. The picture is smaller than the window. The border around it is the part of the screen that
  gets no light: a flat, dark surface.
]]
local PROJECTION = PRECISION .. [[
extern vec2 resolution;
// The pixels of the window for each unit. A rib has the same width on each screen.
extern float scale;
extern float time;
// The border, as a part of the window at each side.
extern float inset;

vec4 effect(mediump vec4 color, Image tex, mediump vec2 uv, mediump vec2 pixel) {
  vec2 sc = love_PixelCoord;
  // The point of the picture. The picture becomes smaller as one unit, with no curve.
  float size = 1.0 - 2.0 * inset;
  vec2 p = (uv - 0.5) / size + 0.5;
  vec2 c = p - 0.5;
  // One unit of the window, as a part of the picture.
  vec2 unit = scale / resolution / size;
  // The three tubes do not agree fully: the red and the blue are a part of a unit apart, and more near the sides.
  vec2 apart = vec2(0.3 + 1.6 * c.x * c.x, 0.0) * unit;
  vec3 col = vec3(Texel(tex, p + apart).r, Texel(tex, p).g, Texel(tex, p - apart).b);
  // The picture is soft, and a bright shape has a light blue halo: six points around the pixel, the far ones at its
  // sides. The halo is on the dark parts only.
  vec3 soft = Texel(tex, p + vec2(1.5, 1.5) * unit).rgb + Texel(tex, p + vec2(-1.5, 1.5) * unit).rgb
    + Texel(tex, p + vec2(1.5, -1.5) * unit).rgb + Texel(tex, p + vec2(-1.5, -1.5) * unit).rgb
    + 0.75 * (Texel(tex, p + vec2(3.5, -0.5) * unit).rgb + Texel(tex, p + vec2(-3.5, 0.5) * unit).rgb);
  soft /= 5.5;
  vec3 weight = vec3(0.299, 0.587, 0.114);
  col = mix(col, soft, 0.10) + vec3(0.62, 0.84, 1.0) * dot(soft, weight) * (1.0 - dot(col, weight)) * 0.16;
  // The hot zone: a wide zone at the middle of the screen. The light falls off toward the top and the bottom, and less
  // toward the sides.
  float hot = exp(-(c.x * c.x * 1.3 + c.y * c.y * 5.0));
  // A small number of wide, soft bands across the screen, and one bar that moves slowly.
  float band = 0.6 * sin(p.y * 31.0 + 0.9) + 0.4 * sin(p.y * 73.0 + 2.1);
  float hum = sin((p.y + time * 0.035) * 6.2831853);
  float tone = clamp(hot + 0.22 * band, 0.0, 1.0);
  // The bands are bands of color: cyan where the light is strong, violet where it is weak.
  col *= mix(vec3(1.05, 0.91, 1.09), vec3(0.95, 1.04, 1.04), tone);
  col *= 0.80 + 0.27 * hot + 0.030 * band + 0.010 * hum;
  // The light of the tubes is also on the dark parts of the picture.
  col += vec3(0.008, 0.020, 0.036) * tone;
  // The fade: one band across the middle of the screen, from side to side, where the picture is pale.
  float fade = exp(-c.y * c.y * 30.0) * (1.0 - 0.6 * c.x * c.x);
  col += (1.0 - col) * vec3(0.060, 0.105, 0.130) * fade;
  // The ribs of the screen: thin lines from the top to the bottom.
  col *= 1.0 - 0.022 * (0.5 + 0.5 * sin(sc.x / scale * 6.2831853 / 3.0));
  // The border: dark gray, and a small amount darker toward the corners of the window. A small part of the light of
  // the picture goes across its edge.
  vec2 w = uv - 0.5;
  vec2 past = max(abs(c) - 0.5, 0.0) * size * resolution / scale;
  vec3 border = vec3(0.040, 0.042, 0.050) * (1.0 - 0.5 * dot(w, w)) + col * 0.10 * exp(-length(past) * 0.22);
  vec2 inside = smoothstep(vec2(0.0), vec2(1.5) / (resolution * size), p) * smoothstep(vec2(0.0), vec2(1.5) / (resolution * size), 1.0 - p);
  return vec4(mix(border, col, inside.x * inside.y), 1.0);
}
]]

-- The post passes, in the sequence of the debug menu. `curve` is the curve of the screen. `inset` is the border around
-- the picture, as a part of the window at each side. The same values move the pointer, thus a click goes to the thing
-- that the player sees.
shaders.POSTS = {
  { id = 'crt', label = 'CRT', note = 'The default: a tube with scan lines', source = CRT, curve = 0.06, inset = 0 },
  { id = 'projection', label = 'Rear projection', note = 'A flat screen in a dark border, with color bands', source = PROJECTION,
    curve = 0, inset = 0.03 },
}
shaders.post = 'crt'

-- The shader of each background and of each post pass that the game showed, by its id.
local backgrounds, posts = {}, {}
local foil
local scene, msaa
-- The time of the frame, for the post pass.
local now = 0
-- The pixels of a canvas for each unit of the window, in each direction.
local density = 1
local pool = {}

function shaders.load()
  foil = lg.newShader(FOIL)
  msaa = math.min(4, lg.getSystemLimits().canvasmsaa)
end

-- Makes the canvases again after a change of the window size.
function shaders.resize()
  local before = density
  density = lg.getDPIScale()
  -- A canvas with no multisampling (WebGL 1) has 2 pixels or more for each unit. The window shows it smaller, thus
  -- the edges are smooth.
  if msaa < 2 then density = math.max(density, 2) end
  local w, h = lg.getDimensions()
  density = math.min(density, lg.getSystemLimits().texturesize / math.max(w, h))
  if density ~= before then
    theme.density = density
    theme.dropFonts()
  end
  scene = lg.newCanvas(w, h, { msaa = msaa, dpiscale = density })
  pool = {}
end

function shaders.current() return shaders.MODES[shaders.mode] end

function shaders.cycle() shaders.mode = shaders.mode % #shaders.MODES + 1 end

local function byId(list, id)
  for _, entry in ipairs(list) do
    if entry.id == id then return entry end
  end
end

function shaders.setBackground(id)
  assert(byId(shaders.BACKGROUNDS, id), 'No background ' .. tostring(id))
  shaders.background = id
end

function shaders.setPost(id)
  assert(byId(shaders.POSTS, id), 'No post pass ' .. tostring(id))
  shaders.post = id
end

-- A background gets its shader when the game shows it for the first time.
local function backgroundShader()
  local id = shaders.background
  local shader = backgrounds[id]
  if not shader then
    shader = lg.newShader(byId(shaders.BACKGROUNDS, id).source)
    if shader:hasUniform('path') then shader:send('path', unpack(TOUR_PATH)) end
    backgrounds[id] = shader
  end
  return shader
end

-- A post pass gets its shader when the game shows it for the first time.
local function postShader()
  local entry = byId(shaders.POSTS, shaders.post)
  local shader = posts[entry.id]
  if not shader then
    shader = lg.newShader(entry.source)
    posts[entry.id] = shader
  end
  return shader, entry
end

function shaders.beginScene(time)
  now = time
  lg.setCanvas({ scene, stencil = true })
  lg.clear(0.082, 0.090, 0.110, 1)
  if shaders.current().background then
    local background = backgroundShader()
    -- A background with no motion or with no picture does not have each value.
    if background:hasUniform('time') then background:send('time', time) end
    if background:hasUniform('resolution') then background:send('resolution', { scene:getPixelDimensions() }) end
    lg.setShader(background)
    lg.setColor(1, 1, 1, 1)
    lg.rectangle('fill', 0, 0, scene:getDimensions())
    lg.setShader()
  end
end

function shaders.endScene()
  lg.setCanvas()
  lg.setColor(1, 1, 1, 1)
  lg.setBlendMode('alpha', 'premultiplied')
  if shaders.current().post then
    local post, entry = postShader()
    post:send('resolution', { lg.getPixelDimensions() })
    post:send('scale', lg.getDPIScale())
    if post:hasUniform('curve') then post:send('curve', entry.curve) end
    if post:hasUniform('time') then post:send('time', now) end
    if post:hasUniform('inset') then post:send('inset', entry.inset) end
    lg.setShader(post)
  end
  lg.draw(scene)
  lg.setShader()
  lg.setBlendMode('alpha')
end

--[[
  Draws a part of the stage with the foil. `area` is a rectangle in stage units, `draw` draws the part, and
  pointerX, pointerY is the pointer in stage units. The part goes to a canvas of its own, and the canvas goes to the
  scene through the foil shader. With the foil off, `draw` draws to the scene.
]]
function shaders.foil(area, time, pointerX, pointerY, strength, draw)
  if not shaders.current().foil then return draw() end
  local u = gfx.u
  -- The canvas starts at a full pixel, thus the part stays sharp.
  local x, y = math.floor(area.x * u) / u, math.floor(area.y * u) / u
  local w, h = math.ceil(area.w * u) + 1, math.ceil(area.h * u) + 1
  local key = w .. 'x' .. h
  local canvas = pool[key]
  if not canvas then
    canvas = lg.newCanvas(w, h, { msaa = msaa, dpiscale = density })
    pool[key] = canvas
  end
  lg.push('all')
  lg.setCanvas({ canvas, stencil = true })
  lg.clear(0, 0, 0, 0)
  lg.origin()
  lg.scale(u)
  lg.translate(-x, -y)
  draw()
  lg.pop()
  foil:send('time', time)
  foil:send('pointer', { ((pointerX or -100) - x) * u / w, ((pointerY or -100) - y) * u / h })
  foil:send('aspect', w / h)
  foil:send('strength', strength)
  lg.setShader(foil)
  lg.setBlendMode('alpha', 'premultiplied')
  lg.setColor(1, 1, 1, 1)
  lg.draw(canvas, x, y, 0, 1 / u)
  lg.setBlendMode('alpha')
  lg.setShader()
end

-- The curve and the border of the post pass in use move a point. With no curve and no border, the point stays.
-- A point in the border goes to a point outside the scene.
local function warp(x, y)
  local entry = byId(shaders.POSTS, shaders.post)
  local cx, cy = x - 0.5, y - 0.5
  local k = (1 + entry.curve * (cx * cx + cy * cy)) / (1 + entry.curve * 0.25) / (1 - 2 * entry.inset)
  return 0.5 + cx * k, 0.5 + cy * k
end

-- The point of the scene that the player sees at a point of the window. The post pass moves the picture.
function shaders.toScene(x, y)
  if not shaders.current().post then return x, y end
  local w, h = lg.getDimensions()
  local sx, sy = warp(x / w, y / h)
  return sx * w, sy * h
end

-- The point of the window that shows a point of the scene. The test script uses it for a click.
function shaders.toWindow(x, y)
  if not shaders.current().post then return x, y end
  local w, h = lg.getDimensions()
  local px, py = x / w, y / h
  for _ = 1, 8 do
    local sx, sy = warp(px, py)
    px, py = px - (sx - x / w), py - (sy - y / h)
  end
  return px * w, py * h
end

return shaders
