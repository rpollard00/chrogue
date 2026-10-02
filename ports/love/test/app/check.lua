-- Acceptance 3: a check. Run with --demo check. Ra1-a8 gives check to the enemy king, then the enemy answers.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/'
return {
  { 'wait', 2.2 },
  { 'click', 'a1' }, { 'click', 'a8' }, { 'wait', 0.2 }, { 'screenshot', out .. 'check-enemy.png' }, { 'dump', out .. 'check-enemy.json' },
  { 'settle' }, { 'dump', out .. 'check-after.json' },
  { 'quit' },
}
