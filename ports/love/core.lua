--[[
  The core in the process of the game: the C interface of core/embed/include/chrogue_core.h.

  core.open(options) gives a core, or nil and the reason. The options are those of chrogue_open: save_dir, seed, debug.
  A core has core:command(line), which gives the response line of a request line (core/PROTOCOL.md), and core:close().

  The WebAssembly build has no LuaJIT. The core is linked into it as the Lua module chrogue_core (ports/web). The
  desktop game loads the library of the core with the FFI of LuaJIT.
]]
local json = require('json')

local core = {}

local NAMES = { Windows = 'chrogue_core.dll', ['OS X'] = 'libchrogue_core.dylib' }

-- The library of the core: CHROGUE_CORE_LIB, else ../../core/target/release from the game folder.
function core.findLibrary()
  local env = os.getenv('CHROGUE_CORE_LIB')
  if env and env ~= '' then return env end
  local source = love.filesystem.getSource():gsub('/+$', '')
  return source .. '/../../core/target/release/' .. (NAMES[love.system.getOS()] or 'libchrogue_core.so')
end

-- The functions open, command and close of the core, from the linked module or from the library.
local function bind()
  local linked, module = pcall(require, 'chrogue_core')
  if linked then return module end
  local ok, ffi = pcall(require, 'ffi')
  if not ok then return nil, 'This build of the game has no core.' end
  ffi.cdef([[
    typedef struct ChrogueCore ChrogueCore;
    ChrogueCore *chrogue_open(const char *options);
    const char *chrogue_open_error(void);
    const char *chrogue_command(ChrogueCore *core, const char *request);
    void chrogue_close(ChrogueCore *core);
  ]])
  local path = core.findLibrary()
  local found, lib = pcall(ffi.load, path)
  if not found then
    return nil, ('The game did not find the library of the core at %s. Build it with "cargo build --release" in core/, or set CHROGUE_CORE_LIB.'):format(path)
  end
  return {
    open = function(options)
      local handle = lib.chrogue_open(options)
      if handle == nil then return nil, ffi.string(lib.chrogue_open_error()) end
      return handle
    end,
    command = function(handle, line) return ffi.string(lib.chrogue_command(handle, line)) end,
    close = function(handle) lib.chrogue_close(handle) end,
  }
end

local Core = {}
Core.__index = Core

function Core:command(line)
  assert(self.handle, 'The core is closed.')
  return self.api.command(self.handle, line)
end

function Core:close()
  if self.handle then self.api.close(self.handle) end
  self.handle = nil
end

function core.open(options)
  local api, problem = bind()
  if not api then return nil, problem end
  local handle, reason = api.open(json.encode(options))
  if not handle then return nil, reason end
  return setmetatable({ api = api, handle = handle }, Core)
end

return core
