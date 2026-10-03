/*
  The Lua module chrogue_core of the WebAssembly build: the C interface of the core (chrogue_core.h) for Lua 5.1.
  The WebAssembly build of LÖVE has no LuaJIT, thus the game cannot load the core with the FFI. build.sh links this file
  and the library of the core into LÖVE. client/core.lua uses the module.

  chrogue_core.open(options) gives a core, or nil and the reason. chrogue_core.command(core, line) gives the response
  line. chrogue_core.close(core) closes the core.
*/
#include <lua.h>
#include <lauxlib.h>

#include "chrogue_core.h"

static ChrogueCore *check_core(lua_State *L) {
  luaL_checktype(L, 1, LUA_TLIGHTUSERDATA);
  return lua_touserdata(L, 1);
}

static int core_open(lua_State *L) {
  ChrogueCore *core = chrogue_open(luaL_checkstring(L, 1));
  if (core == NULL) {
    lua_pushnil(L);
    lua_pushstring(L, chrogue_open_error());
    return 2;
  }
  lua_pushlightuserdata(L, core);
  return 1;
}

static int core_command(lua_State *L) {
  ChrogueCore *core = check_core(L);
  lua_pushstring(L, chrogue_command(core, luaL_checkstring(L, 2)));
  return 1;
}

static int core_close(lua_State *L) {
  chrogue_close(check_core(L));
  return 0;
}

static const luaL_Reg functions[] = {
  { "open", core_open },
  { "command", core_command },
  { "close", core_close },
  { NULL, NULL },
};

int luaopen_chrogue_core(lua_State *L) {
  lua_newtable(L);
  luaL_register(L, NULL, functions);
  return 1;
}
