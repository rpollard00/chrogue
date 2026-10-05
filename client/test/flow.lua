--[[
  A whole session, from the title to the title: a new run, a battle won by clicks (with the promotion picker), the
  result, the camp (take a reward, buy, get new items, move a unit, start), a second won battle, a lost battle, the end
  of the run, the upgrades (buy one), and the title.

  Run with: --seed 7 --debug --no-save --script test/flow.lua (test/run.sh does it at two window sizes).
  The debug commands only prepare positions and the gold of the shop. Each move that the test claims is a click.
  The screenshots and the dumps go to $CHROGUE_OUT (default /tmp/chrogue-love4), with the window size in the name.
]]
local w, h = love.graphics.getDimensions()
local out = (os.getenv('CHROGUE_OUT') or '/tmp/chrogue-love4') .. '/'
local size = ('-%dx%d'):format(w, h)
local function shot(name) return { 'screenshot', out .. name .. size .. '.png' } end
local function dump(name) return { 'dump', out .. name .. size .. '.json' } end

local function events(app)
  local list = {}
  for _, e in ipairs(app.response and app.response.events or {}) do list[#list + 1] = e.type end
  return list
end
local function has(app, kind)
  for _, e in ipairs(app.response and app.response.events or {}) do if e.type == kind then return e end end
end
local function pieceAt(view, s)
  for _, p in ipairs(view.pieces) do if p.square == s then return p end end
end
local function unitOf(view, kind)
  local list = {}
  for _, u in ipairs(view.army) do if u.kind == kind then list[#list + 1] = u end end
  return list
end
local function expect(label, check) return { 'expect', check, label } end
-- With CHROGUE_EFFECTS=off, the script starts with the effects off (two presses of F1), for the test of the layout.
local start = os.getenv('CHROGUE_EFFECTS') == 'off' and { { 'key', 'f1' }, { 'key', 'f1' } } or {}
local function same(a, b) return table.concat(a, ',') == table.concat(b, ',') end

local K, R, B, P, Q = 'k', 'r', 'b', 'p', 'q'

local steps = {
  { 'screen', 'title' }, { 'wait', 0.4 }, shot('title-new'), dump('title-new'),
  expect('the title of a first session', function(v) return v.screen == 'title' and not v.can_continue and v.meta.runs == 0 end),

  -- Battle 1, floor 1: a pawn promotes through the picker, then a bishop takes the last enemy pawn (rout).
  { 'press', 'newRun' }, { 'screen', 'battle' },
  expect('new_run starts the battle of floor 1', function(v, c, app)
    return v.floor.number == 1 and v.phase == 'player' and has(app, 'run_start') and has(app, 'battle_start'), table.concat(events(app), ' ')
  end),
  { 'send', { cmd = 'debug_set_army', units = { { kind = K, home = 4 }, { kind = R, home = 0 }, { kind = B, home = 2 }, { kind = P, home = 8 }, { kind = P, home = 15 } } } },
  { 'send', { cmd = 'debug_set_relic', relic = 'forcedMarch', on = true } },
  { 'send', { cmd = 'debug_set_relic', relic = 'earlyPromo', on = true } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = K, square = 56 }, { kind = P, square = 16 } } } },
  { 'settle' }, { 'wait', 0.45 }, shot('battle-banner'), { 'wait', 1.6 },
  { 'hover', 'medal', 'player', 2 }, { 'wait', 0.4 }, shot('battle-relic-card'), { 'hover', 'none' },
  { 'click', 'h2' }, shot('battle-select'), dump('battle-select'),
  expect('h2 shows its targets', function(v, c) return c.selected == 15 and same(c.targets, { 23, 31 }), table.concat(c.targets, ',') end),
  { 'click', 'h4' }, { 'wait', 0.12 }, shot('battle-thinking'), dump('battle-thinking'),
  expect('after the move, the enemy thinks', function(v, c) return v.phase == 'enemy' and c.status == 'The enemy thinks' and not c.lamp end),
  { 'settle' },
  expect('the enemy moved', function(v, c, app) return v.phase == 'player' and has(app, 'move').color == 'b' end),
  { 'click', 'h4' }, { 'click', 'h6' }, { 'settle' },
  { 'click', 'h6' }, { 'click', 'h7' }, { 'wait', 0.1 }, shot('battle-promo'), dump('battle-promo'),
  expect('the promotion picker has four pieces', function(v, c) return same(c.promotion, { 'q', 'n', 'r', 'b' }) and c.status == 'Select the new piece.' end),
  { 'key', 'escape' },
  expect('Escape closes the picker, and the pawn stays selected', function(v, c) return #c.promotion == 0 and c.selected == 47 end),
  { 'click', 'h7' }, { 'press', 'promo', 'q' }, { 'settle' },
  expect('the pawn is a queen on h7', function(v, c, app) local p = pieceAt(v, 55) return p and p.kind == 'q' and p.id == 5 end),
  { 'click', 'c1' }, { 'click', 'a3' }, { 'settle' }, { 'wait', 2.2 }, shot('battle-victory'), dump('battle-victory'),
  expect('the rout is a victory', function(v)
    local r = v.result
    return v.phase == 'over' and r.outcome == 'victory' and r.reason == 'rout' and r.next == 'camp' and r.rows[1].row == 'captures' and r.total == r.reward.total
  end),

  -- The camp before floor 2.
  { 'press', 'continue' }, { 'screen', 'camp' }, shot('camp-draft'), dump('camp-draft'),
  expect('the camp deals a reward', function(v, c, app)
    return v.floor.number == 2 and v.reward.state == 'open' and not v.can_start and has(app, 'camp_enter').reward == true
  end),
  { 'press', 'take', 1 }, { 'settle' }, shot('camp-taken'), dump('camp-taken'),
  expect('the reward is taken', function(v, c, app)
    local e = has(app, 'camp_action')
    return v.reward.state == 'taken' and v.reward.taken == 0 and v.can_start and e and e.action == 'take_reward'
  end),
  { 'send', { cmd = 'debug_set_gold', gold = 60 } },
  { 'send', { cmd = 'debug_set_shop', offers = { { kind = 'piece', type = 'n' }, { kind = 'relic', id = 'forcedMarch' }, { kind = 'relic', id = 'secondWind' }, { kind = 'piece', type = 'p' } } } },
  { 'settle' }, { 'hover', 'control', 'info', { 'shop', 3 } }, { 'wait', 0.3 }, shot('camp-blocked'), dump('camp-blocked'), { 'hover', 'none' },
  expect('an owned relic is blocked', function(v) return v.shop.offers[2].blocked == 'owned' and v.gold == 60 end),
  { 'press', 'buy', 1 }, { 'settle' }, dump('camp-bought'),
  expect('the knight is bought', function(v, c, app)
    local e = has(app, 'camp_action')
    return e.action == 'buy' and e.gold_before == 60 and v.gold == 60 - 13 and #e.units == 1 and #unitOf(v, 'n') == 1, ('gold %d'):format(v.gold)
  end),
  { 'press', 'reroll' }, { 'wait', 0.15 }, shot('camp-reroll'), { 'settle' }, dump('camp-reroll'),
  expect('the shop has new items', function(v, c, app)
    local e = has(app, 'camp_action')
    return e.action == 'reroll' and e.rolled == true and v.gold == 47 - v.shop.reroll_cost
  end),
  { 'click', 'a1' },
  expect('the rook is selected', function(v, c) return c.selected == 0 end),
  { 'click', 'b1' }, { 'settle' }, shot('camp-moved'), dump('camp-moved'),
  expect('the rook moved to b1', function(v, c, app)
    local e = has(app, 'unit_placed')
    return e and e.from == 0 and e.to == 1 and unitOf(v, 'r')[1].home == 1 and c.selected == -1
  end),
  -- The title in the middle of a run: the camp has no key for it, thus the script sends the command.
  { 'send', { cmd = 'to_title' } }, { 'screen', 'title' }, shot('title-continue'),
  expect('the title can continue the run', function(v, c) return v.can_continue and v.run.floor == 2 and c.keys[1] == 'Continue run (floor 2)' end),
  { 'press', 'continueRun' }, { 'screen', 'camp' },
  { 'press', 'start' }, { 'screen', 'battle' },
  expect('start_battle opens floor 2', function(v) return v.floor.number == 2 and v.phase == 'player' end),

  -- Battle 2, floor 2: Ra1-a8 is checkmate. Before it, the Give up dialog opens and Escape closes it.
  { 'send', { cmd = 'debug_set_army', units = { { kind = K, home = 4 }, { kind = R, home = 0 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = K, square = 63 }, { kind = P, square = 54 }, { kind = P, square = 55 } } } },
  { 'settle' }, { 'wait', 2 },
  { 'press', 'giveUp' }, { 'wait', 0.2 }, shot('battle-giveup'),
  expect('the dialog asks', function(v, c, app) return app.dialog and app.dialog.question == 'The run will end. Give up?' end),
  { 'key', 'escape' },
  expect('Escape closes the dialog', function(v, c, app) return app.dialog == nil and v.phase == 'player' end),
  { 'click', 'a1' }, { 'click', 'a8' }, { 'settle' }, { 'wait', 1.5 },
  expect('checkmate wins floor 2', function(v) return v.result.outcome == 'victory' and v.result.reason == 'checkmate' end),
  { 'key', 'return' }, { 'screen', 'camp' },
  expect('Enter continues to the camp of floor 3', function(v) return v.floor.number == 3 end),
  { 'press', 'skip' }, { 'settle' },
  expect('the reward is skipped', function(v) return v.reward.state == 'skipped' and v.can_start end),
  { 'key', 'return' }, { 'screen', 'battle' },

  -- Battle 3, floor 3: the enemy queen takes the last rook. Your king is alone. Level 6 of the AI does not overlook the capture.
  { 'send', { cmd = 'debug_tune', floor = 3, level = 6 } },
  { 'send', { cmd = 'debug_set_army', units = { { kind = K, home = 4 }, { kind = R, home = 0 } } } },
  { 'send', { cmd = 'debug_set_enemy', pieces = { { kind = K, square = 63 }, { kind = Q, square = 9 } } } },
  { 'settle' }, { 'wait', 2 },
  { 'click', 'e1' }, { 'click', 'f1' }, { 'settle' }, { 'wait', 1.5 }, shot('battle-defeat'), dump('battle-defeat'),
  expect('the rout is a defeat', function(v, c, app)
    local r = v.result
    return r.outcome == 'defeat' and r.reason == 'rout' and r.next == 'lost' and #r.rows == 0 and has(app, 'unit_lost')
  end),
  { 'press', 'continue' }, { 'screen', 'over' }, shot('over-lost'), dump('over-lost'),
  expect('the run ends with 2 floors', function(v, c, app)
    local s = v.summary
    return not s.won and s.cleared == 2 and s.crowns == 2 and v.meta.crowns == 2 and has(app, 'run_end')
  end),

  -- The upgrades: buy Treasury for 2 crowns.
  { 'press', 'upgrades' }, { 'screen', 'upgrades' },
  expect('Treasury is the selected upgrade', function(v, c) return c.selected == 'gold' and c.canBuy end),
  { 'press', 'slot', 'scout' }, { 'wait', 0.1 }, shot('upgrades-scout'),
  expect('Scout is selected and too expensive', function(v, c) return c.selected == 'scout' and not c.canBuy end),
  { 'press', 'slot', 'gold' }, { 'wait', 0.1 }, shot('upgrades-selected'),
  { 'press', 'buy' }, { 'wait', 0.25 }, shot('upgrades-bought'), { 'settle' }, dump('upgrades-bought'),
  expect('Treasury has level 1', function(v, c, app)
    local e = has(app, 'upgrade_bought')
    return e and e.id == 'gold' and e.level == 1 and e.crowns == 0 and v.meta.crowns == 0 and c.level == 1
  end),
  { 'press', 'back' }, { 'screen', 'title' }, shot('title-after'), dump('title-after'),
  expect('the title after the run', function(v) return not v.can_continue and v.meta.runs == 1 and v.meta.best == 2 and v.meta.crowns == 0 end),
  { 'quit' },
}
for i = #start, 1, -1 do table.insert(steps, 1, start[i]) end
return steps
