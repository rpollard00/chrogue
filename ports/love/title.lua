-- The title screen: the port of viewTitle in src/ui/views/title.ts. One column in the center: the wordmark, the menu,
-- and the wells of the crowns, the best run, and the runs.
local gfx = require('gfx')
local layout = require('layout')
local text = require('text')
local theme = require('theme')
local ui = require('ui')
local C, px = theme.color, gfx.px

local title = {}
title.__index = title

local L = layout.title

function title.new(app, view)
  return setmetatable({ app = app, view = view, time = 0 }, title)
end

function title:apply(view) self.view = view end
function title:refused(view) self.view = view end
function title:update(dt) self.time = self.time + dt end
function title:settled() return true end

-- The keys of the menu: name, label, rectangle, kind.
function title:keys()
  local v, m = self.view, L.menu
  if v.can_continue and v.run then
    return {
      { 'continueRun', ('Continue run (floor %d)'):format(v.run.floor), m.primary, 'primary' },
      { 'newRun', 'New run', m.left, 'key' },
      { 'upgrades', 'Upgrades', m.right, 'key' },
    }
  end
  return { { 'newRun', 'New run', m.primary, 'primary' }, { 'upgrades', 'Upgrades', m.full, 'key' } }
end

function title:hit(x, y)
  for _, k in ipairs(self:keys()) do
    if layout.contains(k[3], x, y) then return k[1] end
  end
end

function title:control(name)
  for _, k in ipairs(self:keys()) do
    if k[1] == name then return k[3] end
  end
end

function title:activate(name)
  local app = self.app
  if app.waiting() then return end
  if name == 'continueRun' then app.send({ cmd = 'continue_run' })
  elseif name == 'upgrades' then app.send({ cmd = 'open_upgrades' })
  elseif name == 'newRun' then
    -- A new run replaces the saved run, with no crowns for it. The web game asks first.
    if self.view.can_continue then app.confirm(text.NEW_RUN, function() app.send({ cmd = 'new_run' }) end)
    else app.send({ cmd = 'new_run' }) end
  end
end

function title:key(key)
  if key == 'return' or key == 'kpenter' then
    self:activate(self:keys()[1][1])
    return true
  end
  return false
end

local function wordmark(r)
  local size = 6
  local opts = { align = 'center', width = r.w, line = r.h, tracking = 0.05 }
  local word = 'CHROGUE'
  -- text-shadow: 0 -1px shine, 0 3px edge, 0 10px 14px shade.
  for i = 1, 6 do
    opts.color, opts.alpha = C.black, 0.13
    local spread = (i - 3.5) * 0.12
    gfx.text(word, 'display', size, r.x + spread, r.y + 0.62 + spread * 0.5, opts)
  end
  opts.alpha = nil
  opts.color = C.shineHard
  gfx.text(word, 'display', size, r.x, r.y - px(1), opts)
  opts.color = C.edge
  gfx.text(word, 'display', size, r.x, r.y + px(3), opts)
  opts.color = C.text
  gfx.text(word, 'display', size, r.x, r.y, opts)
end

-- A well with a caption and a value.
local function stat(r, caption, draw)
  gfx.well(r.x, r.y, r.w, r.h, px(9))
  gfx.text(caption:upper(), 'bold', 0.66, r.x, r.y + 0.4, { color = C.dim, tracking = 0.1, align = 'center', width = r.w, line = 0.99 })
  draw(r.x, r.y + 1.39, r.w, 1.44)
end

function title:draw(pointer)
  local v = self.view
  wordmark(L.wordmark)
  gfx.text(text.TAGLINE, 'body', 1, L.tagline.x, L.tagline.y, { color = C.dim, align = 'center', width = L.tagline.w, line = L.tagline.h })
  local bar = L.menu.bar
  gfx.raised(bar.x, bar.y, bar.w, bar.h, px(12))
  for _, k in ipairs(self:keys()) do ui.key(k[3], k[2], k[4], ui.state(pointer, k[1])) end
  local w = L.stats.wells
  local meta = v.meta
  stat(w[1], 'Crowns', function(x, y, width, line)
    local opts = { size = 1.2, face = 'display', color = C.text, tracking = 0.03 }
    local aw = ui.amountWidth(meta.crowns, opts)
    ui.amount('crowns', meta.crowns, x + (width - aw) / 2, y, line, opts)
  end)
  stat(w[2], 'Best', function(x, y, width, line)
    gfx.text(('%d of %d floors'):format(meta.best, v.floors), 'display', 1.2, x, y, { align = 'center', width = width, line = line, tracking = 0.03 })
  end)
  stat(w[3], 'Runs', function(x, y, width, line)
    gfx.text(tostring(meta.runs), 'display', 1.2, x, y, { align = 'center', width = width, line = line, tracking = 0.03 })
  end)
end

function title:state()
  local keys = {}
  for _, k in ipairs(self:keys()) do keys[#keys + 1] = k[2] end
  return { keys = keys }
end

return title
