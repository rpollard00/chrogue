-- The interface text. The core sends codes (core/PROTOCOL.md, "Codes for the
-- client text"), and this module selects the text for each code. The content text (names and texts of relics, upgrades,
-- floors, pieces, and offers) comes from the views of the core.
local text = {}

text.WIN = {
  w = {
    checkmate = 'Checkmate. You won the battle.',
    rout = 'The enemy king is alone. You won the battle.',
    stalemate = 'The enemy has no legal move. You won the battle.',
  },
  b = {
    checkmate = 'Checkmate. The run ends.',
    rout = 'Your king is alone. The run ends.',
    stalemate = 'You have no legal move. The run ends.',
  },
}
text.DRAW = {
  clock = '50 moves passed with no capture. The battle is a draw.',
  bare = 'Only the kings remain. The battle is a draw.',
}
text.OUTCOME = { victory = 'Victory', defeat = 'Defeat', draw = 'Draw' }

-- The sentence that tells the cause of the result of a battle.
function text.cause(result)
  if result.winner == nil then return text.DRAW[result.reason] or result.reason end
  return (text.WIN[result.winner] or {})[result.reason] or result.reason
end

-- The label of a row of the result panel.
function text.resultRow(row)
  if row.row == 'captures' then return 'Gold from captures' end
  if row.row == 'clear' then return 'Gold for the win' end
  return 'Gold from ' .. tostring(row.label)
end

-- The label of a row of the tally at the end of a run.
function text.overRow(row)
  if row.row == 'win' then return 'Crowns for the win' end
  return 'Crowns for the floors'
end

text.BLOCKED = { army_full = 'Army full', owned = 'Owned', relics_full = 'Relics full' }
text.KIND = { piece = 'Unit', relic = 'Relic', gold = 'Gold' }
text.REWARD_HEAD = { open = 'Select one reward', taken = 'Reward taken', skipped = 'Reward skipped' }

function text.floor(floor) return ('Floor %d of %d'):format(floor.number, floor.total) end

text.STATUS = {
  move = 'Your move.',
  promotion = 'Select the new piece.',
  enemy = 'The enemy thinks',
  check = 'Your king is in check.',
  -- A battle can start with no legal move for the player (core/PROTOCOL.md, "Open issues"). Only Give up ends it.
  stuck = 'No legal move. Give up.',
  -- The core refused enemy_move two times.
  enemyFailed = 'The enemy move failed.',
}

text.GIVE_UP = 'The run will end. Give up?'
text.NEW_RUN = 'Your current run will end. Start a new run?'
text.TAGLINE = 'Eight battles. One army. Every piece you lose stays lost.'
text.ARMY_HINT = 'To move a piece, select the piece and then select a square.'
text.START_REASON = 'Select or skip the reward before the battle.'
text.UPGRADES_HINT = 'Upgrades apply to each new run.'

-- The text of the connection to the core.
text.NET = {
  starting = 'The game starts the core.',
  connecting = 'The game connects to the core.',
  auth = 'The game connects to the core.',
  -- Connected, and the first view has not come.
  connected = 'The game waits for the core.',
  lost = 'The connection to the core is lost. The game tries again.',
  retry = 'Attempt %d',
}

-- The warnings about the saved data.
local SAVED = { meta = 'crowns and upgrades', run = 'run' }
local PROBLEM = {
  unreadable = 'The core cannot read the saved %s.',
  invalid = 'The saved %s has data that the core cannot use.',
  newer_version = 'A newer version of the core wrote the saved %s.',
}
local AFTER = { meta = 'The game continues with no crowns and no upgrades.', run = 'The game continues with no saved run.' }

--[[
  The notice for an event of a response, or nil. A notice has `kind`, `title`, `lines` (sentences), and `detail` (the
  message of the core, or nil). `dir` is the save folder, thus the notice can give the full path of a file.
]]
function text.notice(e, dir)
  local what = SAVED[e.what] or tostring(e.what)
  if e.type == 'save_failed' then
    return { kind = 'failed', title = 'The game did not save', detail = e.message,
      lines = { ('The core cannot write the saved %s. The game continues, but if it stops, the changes since the last save are lost.'):format(what) } }
  elseif e.type == 'save_problem' then
    local lines = { (PROBLEM[e.reason] or 'The core cannot use the saved %s.'):format(what) }
    local kept = type(e.kept) == 'string' and e.kept or nil
    if kept then
      lines[2] = ('The core kept the old file as %s.'):format(kept:find('^/') and kept or (dir and dir .. '/' .. kept or kept))
      lines[3] = AFTER[e.what]
    else
      lines[2] = ('The core cannot move the old file, thus it does not save the %s.'):format(what)
    end
    return { kind = kept and 'problem' or 'failed', title = 'The game cannot use a saved file', lines = lines, detail = e.message }
  end
end

return text
