-- Acceptance 3: the promotion picker. Run with --demo promo.
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-ports/love') .. '/'
return {
  { 'wait', 2.2 },
  { 'click', 'a7' }, { 'click', 'a8' }, { 'wait', 0.2 }, { 'screenshot', out .. 'promo-picker.png' }, { 'dump', out .. 'promo-picker.json' },
  -- A click on a square does nothing while the picker is open.
  { 'click', 'e1' }, { 'dump', out .. 'promo-blocked.json' },
  { 'press', 'promo', 1 }, { 'wait', 0.33 }, { 'screenshot', out .. 'promo-done.png' },
  { 'settle' }, { 'dump', out .. 'promo-done.json' },
  { 'quit' },
}
