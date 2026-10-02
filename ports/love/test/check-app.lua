-- Compares the state files that test/app.sh saved with the state that each script must give.
local dir = arg[1] or '/tmp/chrogue-ports/love'
local failed, checked = 0, 0

local function read(name)
  local file = assert(io.open(dir .. '/' .. name .. '.json'), 'No state file: ' .. name)
  local text = file:read('*a')
  file:close()
  return text
end

-- The value of the first key with a name, as text.
local function value(text, key)
  local found = text:match('"' .. key .. '": ([^\n]-),?\n')
  return found and found:gsub('^"(.*)"$', '%1') or nil
end

local function expect(name, pairs_)
  local text = read(name)
  for i = 1, #pairs_, 2 do
    local key, wanted = pairs_[i], tostring(pairs_[i + 1])
    local got = value(text, key)
    checked = checked + 1
    if got ~= wanted then
      failed = failed + 1
      print(('FAIL %s: %s is %s, expected %s'):format(name, key, tostring(got), wanted))
    end
  end
end

local START = 'rnb1k2r/3pp3/8/8/8/8/2PPPP2/R3K1N1'
local END = '1k6/3p4/b7/4p3/2P1P3/3PK3/5P2/4r3'

-- play.lua, with --seed 7
expect('play-start', { 'board', START, 'status', 'Your move.', 'lamp', true, 'enemy', 'The Warden', 'floor', 4, 'runGold', 24, 'canScout', true,
  'relics', '["forcedMarch", "backpedal", "earlyPromo"]', 'traits', '["kingKnight"]', 'effects', 'Effects: all on', 'width', 1280, 'height', 720 })
expect('play-select', { 'selected', 12, 'targets', '[20, 28]', 'scouting', false })
expect('play-scout', { 'selected', 57, 'targets', '[40, 42]', 'scouting', true })
expect('play-card', { 'playerMedal', 2, 'selected', -1 })
expect('play-thinking', { 'busy', true, 'status', 'The enemy thinks', 'lamp', false, 'moves', 1, 'captureGold', 5, 'w', 'r' })
expect('play-1', { 'busy', false, 'moves', 2, 'board', 'Rnb1k3/3pp3/8/8/8/8/2PPPP2/4K1Nr' })
expect('play-2', { 'check', 4, 'status', 'Your king is in check.', 'lamp', true, 'moves', 4, 'b', 'n' })
expect('play-end', { 'board', END, 'moves', 14, 'captureGold', 8, 'w', 'rn', 'b', 'nr', 'check', 20, 'result', 'none' })
expect('play-f1-2', { 'effects', 'Effects: post pass off' })
expect('play-f1-3', { 'effects', 'Effects: all off' })
expect('play-f1-1', { 'effects', 'Effects: all on' })
expect('play-confirm', { 'confirm', true, 'board', END })
expect('play-cancel', { 'confirm', false, 'board', END, 'moves', 14 })
expect('play-restart', { 'confirm', false, 'board', START, 'moves', 0, 'captureGold', 0 })

-- mate.lua, with --demo mate
expect('mate-start', { 'board', '7k/6pp/8/8/8/8/2P5/R3K3', 'runGold', 27, 'floor', 3 })
-- The win of floor 3 gives 6 gold. Interest gives floor((27 + 6) / 5) = 6 gold.
expect('mate-result', { 'winner', 'w', 'reason', 'checkmate', 'captures', 0, 'clear', 6, 'gold', 6, 'label', 'Interest', 'total', 12, 'lamp', false })
expect('mate-blocked', { 'selected', -1, 'moves', 1 })
expect('mate-continue', { 'board', '7k/6pp/8/8/8/8/2P5/R3K3', 'moves', 0, 'result', 'none', 'status', 'Your move.' })

-- promo.lua, with --demo promo
expect('promo-picker', { 'promotion', 4, 'status', 'Select the new piece.', 'moves', 0, 'board', '8/P7/7k/7p/8/8/8/4K3' })
expect('promo-blocked', { 'promotion', 4, 'selected', 48, 'moves', 0 })
expect('promo-done', { 'promotion', 0, 'moves', 2 })
checked = checked + 1
if not value(read('promo-done'), 'board'):find('^Q7/') then
  failed = failed + 1
  print('FAIL promo-done: no queen on a8: ' .. value(read('promo-done'), 'board'))
end

-- check.lua, with --demo check
expect('check-enemy', { 'check', 60, 'busy', true, 'moves', 1 })
expect('check-after', { 'busy', false, 'moves', 2 })

-- ends.lua, with --demo defeat and --demo draw
expect('defeat', { 'winner', 'b', 'reason', 'rout', 'total', 0, 'b', 'r' })
expect('draw', { 'winner', 'none', 'reason', 'bare', 'captures', 1, 'clear', 0, 'total', 1 })

-- start.lua at the two other window sizes
expect('start-1440x900', { 'width', 1440, 'height', 900, 'unit', 18, 'board', START })
expect('start-1920x1080', { 'width', 1920, 'height', 1080, 'unit', 24, 'board', START })

-- The frames and the enemy search of the long script.
local text = read('play-end')
print(('play.lua: %s frames per second on average, enemy search: worst %s ms, average %s ms in %s searches'):format(
  value(text, 'average'), value(text, 'worstMs'), value(text, 'averageMs'), value(text, 'count')))
print(('%d values checked, %d failed'):format(checked, failed))
os.exit(failed == 0 and 0 or 1)
