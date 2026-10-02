-- The functions that the generated rules need and that the TypeScriptToLua library does not have.
-- Load this file before a module of generated/. The generated/ folder must be in the module path.
-- tools/prepare-rules.ts and tools/tstl-plugin.cjs tell why each function exists.

local maxn, unpack = table.maxn, unpack

-- The condition of JavaScript: 0, NaN, '', false, and nil are false.
function __truthy(value)
  return value ~= nil and value ~= false and value ~= 0 and value ~= '' and value == value
end
__present = __truthy

local function nextSparse(list, i)
  if i < list.last then return i + 1, list.items[i + 1] end
end

-- As ipairs, but the loop does not stop at a nil item. It stops after the last item that is not nil.
function __chrogue_sparse_ipairs(items)
  return nextSparse, { items = items, last = maxn(items) }, 0
end

function __chrogue_sparse_unpack(items)
  return unpack(items, 1, maxn(items))
end

-- Array(n) in the source makes a list of n empty items. A Lua table has no length of its own, thus the list is empty.
Array = setmetatable({}, { __call = function() return {} end })

-- The sort of JavaScript keeps the sequence of equal items, and table.sort does not. The enemy AI sorts its moves,
-- and it selects the first of the moves with the best score. Thus a different sort gives a different move.
-- This is an insertion sort: a list of moves is short.
local lualib = require('lualib_bundle')
local librarySort = lualib.__TS__ArraySort
function lualib.__TS__ArraySort(list, compare)
  if not compare then return librarySort(list, compare) end
  for i = 2, #list do
    local item, j = list[i], i - 1
    while j >= 1 and compare(nil, list[j], item) > 0 do
      list[j + 1] = list[j]
      j = j - 1
    end
    list[j + 1] = item
  end
  return list
end

-- Object.assign of the library stops at a source that is nil. This one reads each source.
function lualib.__TS__ObjectAssign(target, ...)
  for i = 1, select('#', ...) do
    local source = select(i, ...)
    if type(source) == 'table' then
      for key, value in pairs(source) do target[key] = value end
    end
  end
  return target
end
