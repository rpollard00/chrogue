--[[
  The notices about the saved data. The save folder cannot take a file, thus the core sends save_failed. The script then
  gives a save_problem event, as a core sends it when a saved file is bad and the core kept it aside.
  Run with: --seed 7 --save-dir READ_ONLY_FOLDER --script test/saves.lua
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function expect(label, check) return { 'expect', check, label } end
local function kinds(app)
  local list = {}
  for i, n in ipairs(app.notices) do list[i] = n.kind .. ':' .. n.title end
  return table.concat(list, ', ')
end
local count
return {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' }, { 'wait', 2 },
  expect('save_failed gives a notice, and the battle continues', function(v, c, app)
    count = #app.notices
    return count >= 1 and app.notices[1].kind == 'failed' and v.phase == 'player', kinds(app)
  end),
  { 'click', 'e2' }, { 'click', 'e4' }, { 'settle' },
  expect('the player can move while the notice shows', function(v) return v.last ~= nil end),
  shot('notice-save-failed'),
  { 'events', { { type = 'save_problem', what = 'run', reason = 'unreadable', message = 'run.json: expected value at line 1 column 1', kept = 'run.json.bad-1790000000' } } },
  { 'wait', 0.4 }, shot('notice-save-problem'), { 'dump', out .. 'notice' .. size .. '.json' },
  expect('save_problem gives a notice with the path of the old file', function(v, c, app)
    local n = app.notices[#app.notices]
    local text = table.concat(n.lines, ' ')
    return #app.notices == count + 1 and n.kind == 'problem' and text:find(app.saveDir .. '/run.json.bad-1790000000', 1, true) ~= nil, text
  end),
  { 'press', 'notice', 1 }, { 'wait', 0.1 },
  expect('a click on a notice closes it', function(v, c, app) return #app.notices == count end),
  { 'quit' },
}
