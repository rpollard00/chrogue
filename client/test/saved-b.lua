--[[
  The second half of the test of the saved run: a new game on the same save folder finds the run, and continues its
  battle. Run after test/saved-a.lua with the same --save-dir.
]]
return {
  { 'screen', 'title' },
  { 'expect', function(v) return v.can_continue and v.run.floor == 1 and v.run.phase == 'battle' end, 'the title has the saved run' },
  { 'press', 'continueRun' }, { 'screen', 'battle' },
  { 'expect', function(v) return v.floor.number == 1 and v.phase == 'player' end, 'the battle of the saved run continues' },
  { 'quit' },
}
