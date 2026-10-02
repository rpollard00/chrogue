-- The frames per second with no limit from the display. Run with --novsync. A relic card shows, thus each shader does work.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/' .. (os.getenv('CHROGUE_NAME') or 'fps')
return {
  { 'wait', 2.2 }, { 'hover', 'player', 2 }, { 'wait', 4 }, { 'dump', out .. '.json' },
  { 'key', 'f1' }, { 'wait', 3 }, { 'dump', out .. '-post-off.json' },
  { 'key', 'f1' }, { 'wait', 3 }, { 'dump', out .. '-all-off.json' },
  { 'quit' },
}
