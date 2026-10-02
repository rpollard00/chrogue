-- A defeat or a draw. Run with --demo defeat or --demo draw. CHROGUE_NAME is the name of the files.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/' .. (os.getenv('CHROGUE_NAME') or 'end')
local first = os.getenv('CHROGUE_NAME') == 'draw' and { 'e1', 'e2' } or { 'a8', 'g8' }
return {
  { 'wait', 2.2 },
  { 'click', first[1] }, { 'click', first[2] }, { 'settle' }, { 'wait', 2.0 },
  { 'screenshot', out .. '-panel.png' }, { 'dump', out .. '.json' },
  { 'quit' },
}
