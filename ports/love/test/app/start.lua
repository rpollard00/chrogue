-- Acceptance 4: the first position at a window size. CHROGUE_NAME is the name of the files.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/' .. (os.getenv('CHROGUE_NAME') or 'start')
return {
  { 'wait', 0.7 }, { 'screenshot', out .. '-intro.png' },
  { 'wait', 1.8 }, { 'hover', 'player', 3 }, { 'wait', 0.5 }, { 'screenshot', out .. '-card.png' },
  { 'hover', 'none' }, { 'wait', 0.3 }, { 'screenshot', out .. '.png' }, { 'dump', out .. '.json' },
  { 'quit' },
}
