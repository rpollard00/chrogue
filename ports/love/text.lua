-- The interface text. The strings are the strings of src/ui. The core sends codes (core/PROTOCOL.md, "Codes for the
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
  clock = '50 moves passed with no capture and no pawn advance. The battle is a draw.',
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

text.BLOCKED = { army_full = 'Army full', owned = 'Owned' }
text.KIND = { piece = 'Unit', relic = 'Relic', gold = 'Gold' }
text.REWARD_HEAD = { open = 'Select one reward', taken = 'Reward taken', skipped = 'Reward skipped' }

function text.floor(floor) return ('Floor %d of %d'):format(floor.number, floor.total) end

text.STATUS = {
  move = 'Your move.',
  promotion = 'Select the new piece.',
  enemy = 'The enemy thinks',
  check = 'Your king is in check.',
}

text.GIVE_UP = 'The run will end. Give up?'
text.NEW_RUN = 'Your current run will end. Start a new run?'
text.TAGLINE = 'Eight battles. One army. Every piece you lose stays lost.'
text.ARMY_HINT = 'To move a piece, select the piece and then select a square.'
text.START_REASON = 'Select or skip the reward before the battle.'
text.UPGRADES_HINT = 'Upgrades apply to each new run.'

-- The text of the connection to the core. The web game has no core, thus these strings are new.
text.NET = {
  starting = 'The game starts the core.',
  connecting = 'The game connects to the core.',
  lost = 'The connection to the core is lost. The game tries again.',
  retry = 'Attempt %d',
}

return text
