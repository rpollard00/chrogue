-- The three shaders of the experiment, and the canvases that they need.
-- 1. background: a slow swirl behind the layout.
-- 2. post: the full scene goes to a canvas, and the canvas goes to the window through this shader.
-- 3. foil: a sheen on the relic medals and on the relic card. It moves with the time and with the pointer.
local gfx = require('gfx')
local lg = love.graphics

local shaders = {}

-- F1 changes the mode.
shaders.MODES = {
  { label = 'Effects: all on', background = true, foil = true, post = true },
  { label = 'Effects: post pass off', background = true, foil = true, post = false },
  { label = 'Effects: all off', background = false, foil = false, post = false },
}
shaders.mode = 1

-- The curve of the screen in the post pass. The same value moves the pointer, thus a click goes to the thing that the player sees.
local CURVE = 0.06

local BACKGROUND = [[
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

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 sc) {
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

local FOIL = [[
extern float time;
extern vec2 pointer;
extern float aspect;
extern float strength;

vec3 rainbow(float h) { return clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0); }

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 sc) {
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

local POST = [[
extern vec2 resolution;
extern float curve;

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 sc) {
  // The curve of a tube screen: a point is moved away from the center, and more near the corners.
  vec2 c = uv - 0.5;
  vec2 w = 0.5 + c * (1.0 + curve * dot(c, c)) / (1.0 + curve * 0.25);
  c = w - 0.5;
  // The red and the blue come from positions that are a small distance apart. The distance is 0 at the center.
  vec2 apart = c * dot(c, c) * 0.006;
  vec3 col = vec3(Texel(tex, w + apart).r, Texel(tex, w).g, Texel(tex, w - apart).b);
  // The scan lines are light, thus the text stays easy to read.
  float scan = 0.5 + 0.5 * sin(sc.y * 6.2831853 / 3.0);
  col *= 1.025 - 0.055 * scan;
  col *= 1.0 - 0.32 * smoothstep(0.38, 0.86, length(c));
  vec2 inside = smoothstep(vec2(0.0), vec2(1.5) / resolution, w) * smoothstep(vec2(0.0), vec2(1.5) / resolution, 1.0 - w);
  return vec4(col * inside.x * inside.y, 1.0);
}
]]

local background, foil, post
local scene, msaa
local pool = {}

function shaders.load()
  background = lg.newShader(BACKGROUND)
  foil = lg.newShader(FOIL)
  post = lg.newShader(POST)
  msaa = math.min(4, lg.getSystemLimits().canvasmsaa)
end

-- Makes the canvases again after a change of the window size.
function shaders.resize()
  scene = lg.newCanvas(lg.getWidth(), lg.getHeight(), { msaa = msaa })
  pool = {}
end

function shaders.current() return shaders.MODES[shaders.mode] end

function shaders.cycle() shaders.mode = shaders.mode % #shaders.MODES + 1 end

function shaders.beginScene(time)
  lg.setCanvas({ scene, stencil = true })
  lg.clear(0.082, 0.090, 0.110, 1)
  if shaders.current().background then
    background:send('time', time)
    background:send('resolution', { scene:getDimensions() })
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
    post:send('resolution', { scene:getDimensions() })
    post:send('curve', CURVE)
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
    canvas = lg.newCanvas(w, h, { msaa = msaa })
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

local function warp(x, y)
  local cx, cy = x - 0.5, y - 0.5
  local k = (1 + CURVE * (cx * cx + cy * cy)) / (1 + CURVE * 0.25)
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
