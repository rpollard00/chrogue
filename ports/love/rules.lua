-- The rules of the game. generated/ has the Lua that TypeScriptToLua makes from src/engine and src/game.
-- The port does not have rules of its own.
love.filesystem.setRequirePath('?.lua;?/init.lua;generated/?.lua')

if not love.filesystem.getInfo('generated/engine/index.lua') then
  error('The generated rules are not there. Run `bun install` and `bun run build:lua` in ports/love.')
end

require('rules_runtime')

return {
  engine = require('engine.index'),
  battle = require('game.battle'),
  relics = require('game.relics'),
  floors = require('game.floors'),
  army = require('game.army'),
  upgrades = require('game.upgrades'),
}
