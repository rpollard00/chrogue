-- Acceptance 2: the result panel, and Continue starts the battle again. Run with --demo mate.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/'
return {
  { 'wait', 2.2 }, { 'dump', out .. 'mate-start.json' },
  { 'click', 'a1' }, { 'click', 'a8' }, { 'wait', 0.5 }, { 'screenshot', out .. 'result-start.png' },
  { 'wait', 2.0 }, { 'screenshot', out .. 'result-panel.png' }, { 'dump', out .. 'mate-result.json' },
  -- A click on the board does nothing while the result shows.
  { 'click', 'e1' }, { 'dump', out .. 'mate-blocked.json' },
  { 'press', 'continue' }, { 'wait', 2.2 }, { 'dump', out .. 'mate-continue.json' }, { 'screenshot', out .. 'result-continue.png' },
  { 'quit' },
}
