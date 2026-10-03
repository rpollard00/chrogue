--[[
  The frames per second on the camp and the battle, in each effects mode. The camp has its largest content (the last
  boss, a full army, 10 relics) and a relic card under the pointer; the battle has 10 relics and the boss.
  Run with: --size 1920x1080 --novsync --seed 7 --debug --no-save --script test/fps.lua
]]
local RELICS = { 'forcedMarch', 'backpedal', 'earlyPromo', 'kingKnight', 'longLeap', 'sidestep', 'bounty', 'secondWind', 'conscription', 'interest' }
local steps = {
  { 'screen', 'title' }, { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'send', { cmd = 'debug_set_floor', floor = 7 } },
}
for _, id in ipairs(RELICS) do steps[#steps + 1] = { 'send', { cmd = 'debug_set_relic', relic = id, on = true } } end
local function modes(label)
  return {
    { 'wait', 1 }, { 'fps', 4, label },
    { 'key', 'f1' }, { 'wait', 0.5 }, { 'fps', 3, label },
    { 'key', 'f1' }, { 'wait', 0.5 }, { 'fps', 3, label },
    { 'key', 'f1' },
  }
end
for _, s in ipairs({
  { 'send', { cmd = 'debug_set_army', units = { { kind = 'k', home = 4 }, { kind = 'r', home = 7 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = 'k', square = 56 }, { kind = 'p', square = 48 }, { kind = 'p', square = 49 } } } },
  { 'settle' }, { 'wait', 2 }, { 'click', 'h1' }, { 'click', 'h8' }, { 'settle' }, { 'wait', 1 },
  { 'press', 'continue' }, { 'screen', 'camp' }, { 'hover', 'medal', 'player', 10 },
}) do steps[#steps + 1] = s end
for _, s in ipairs(modes('camp')) do steps[#steps + 1] = s end
for _, s in ipairs({
  { 'hover', 'none' }, { 'press', 'skip' }, { 'settle' }, { 'press', 'start' }, { 'screen', 'battle' }, { 'wait', 2 },
  { 'hover', 'medal', 'player', 10 },
}) do steps[#steps + 1] = s end
for _, s in ipairs(modes('battle')) do steps[#steps + 1] = s end
steps[#steps + 1] = { 'quit' }
return steps
