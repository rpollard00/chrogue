--[[
  The first half of the test of the saved run: a new run starts its first battle, and the game quits. The core saves the
  run when the battle starts. Run with a save folder: --seed 7 --save-dir PATH --script test/saved-a.lua.
]]
return {
  { 'screen', 'title' },
  { 'expect', function(v) return not v.can_continue end, 'the save folder has no run' },
  { 'press', 'newRun' }, { 'screen', 'battle' },
  { 'expect', function(v) return v.floor.number == 1 and v.phase == 'player' end, 'the first battle started' },
  { 'quit' },
}
