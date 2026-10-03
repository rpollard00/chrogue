-- A small JSON reader and writer for the lines of the core protocol (core/PROTOCOL.md).
-- A null in an object is a missing key. A null in a list is json.null, thus the list keeps its length
-- (the 16 slots of the upgrades view have nulls).
local json = {}

json.null = setmetatable({}, { __tostring = function() return 'null' end })

local escapes = { ['"'] = '"', ['\\'] = '\\', ['/'] = '/', b = '\b', f = '\f', n = '\n', r = '\r', t = '\t' }

local function fail(text, at, what) error(('JSON: %s at byte %d: %s'):format(what, at, text:sub(at, at + 20)), 0) end

local function skip(text, at) return text:find('[^ \t\r\n]', at) or #text + 1 end

local value

local function utf8char(code)
  if code < 0x80 then return string.char(code) end
  if code < 0x800 then return string.char(0xC0 + math.floor(code / 0x40), 0x80 + code % 0x40) end
  if code < 0x10000 then
    return string.char(0xE0 + math.floor(code / 0x1000), 0x80 + math.floor(code / 0x40) % 0x40, 0x80 + code % 0x40)
  end
  return string.char(0xF0 + math.floor(code / 0x40000), 0x80 + math.floor(code / 0x1000) % 0x40,
    0x80 + math.floor(code / 0x40) % 0x40, 0x80 + code % 0x40)
end

local function str(text, at)
  local parts, i = {}, at + 1
  while true do
    local j = text:find('["\\]', i)
    if not j then fail(text, at, 'unterminated string') end
    parts[#parts + 1] = text:sub(i, j - 1)
    if text:sub(j, j) == '"' then return table.concat(parts), j + 1 end
    local e = text:sub(j + 1, j + 1)
    if e == 'u' then
      local code = tonumber(text:sub(j + 2, j + 5), 16) or fail(text, j, 'bad escape')
      local after = j + 6
      -- A pair of surrogates is one character.
      if code >= 0xD800 and code < 0xDC00 and text:sub(after, after + 1) == '\\u' then
        local low = tonumber(text:sub(after + 2, after + 5), 16)
        if low and low >= 0xDC00 and low < 0xE000 then
          code = 0x10000 + (code - 0xD800) * 0x400 + (low - 0xDC00)
          after = after + 6
        end
      end
      parts[#parts + 1] = utf8char(code)
      i = after
    else
      parts[#parts + 1] = escapes[e] or fail(text, j, 'bad escape')
      i = j + 2
    end
  end
end

function value(text, at)
  at = skip(text, at)
  local c = text:sub(at, at)
  if c == '{' then
    local result = {}
    at = skip(text, at + 1)
    if text:sub(at, at) == '}' then return result, at + 1 end
    while true do
      if text:sub(at, at) ~= '"' then fail(text, at, 'expected a key') end
      local key
      key, at = str(text, at)
      at = skip(text, at)
      if text:sub(at, at) ~= ':' then fail(text, at, 'expected :') end
      local v
      v, at = value(text, at + 1)
      if v ~= json.null then result[key] = v end
      at = skip(text, at)
      local d = text:sub(at, at)
      if d == '}' then return result, at + 1 end
      if d ~= ',' then fail(text, at, 'expected , or }') end
      at = skip(text, at + 1)
    end
  elseif c == '[' then
    local result = {}
    at = skip(text, at + 1)
    if text:sub(at, at) == ']' then return result, at + 1 end
    while true do
      local v
      v, at = value(text, at)
      result[#result + 1] = v
      at = skip(text, at)
      local d = text:sub(at, at)
      if d == ']' then return result, at + 1 end
      if d ~= ',' then fail(text, at, 'expected , or ]') end
      at = at + 1
    end
  elseif c == '"' then
    return str(text, at)
  elseif text:find('^true', at) then
    return true, at + 4
  elseif text:find('^false', at) then
    return false, at + 5
  elseif text:find('^null', at) then
    return json.null, at + 4
  end
  local number = text:match('^-?%d+%.?%d*[eE]?[-+]?%d*', at)
  if not number or number == '' then fail(text, at, 'unexpected character') end
  return tonumber(number), at + #number
end

function json.decode(text)
  local result, at = value(text, 1)
  at = skip(text, at)
  if at <= #text then fail(text, at, 'text after the value') end
  return result
end

local function isList(t)
  if next(t) == nil then return true end
  local n = 0
  for _ in pairs(t) do n = n + 1 end
  return n == #t
end

local function quote(s)
  return '"' .. s:gsub('[%c"\\]', function(ch)
    if ch == '"' then return '\\"' elseif ch == '\\' then return '\\\\' elseif ch == '\n' then return '\\n' end
    return ('\\u%04x'):format(ch:byte())
  end) .. '"'
end

-- Writes a value. With `indent`, the text has one item on each line, and the keys are sorted. An empty table is a list.
function json.encode(v, indent, depth)
  depth = depth or 0
  local kind = type(v)
  if v == nil or v == json.null then return 'null' end
  if kind == 'boolean' then return tostring(v) end
  if kind == 'number' then
    if v ~= v or v == math.huge or v == -math.huge then return 'null' end
    if v == math.floor(v) and math.abs(v) < 1e15 then return ('%d'):format(v) end
    return ('%.6g'):format(v)
  end
  if kind == 'string' then return quote(v) end
  if kind ~= 'table' then return quote(tostring(v)) end
  local nl, pad, inner = '', '', ''
  if indent then nl, pad, inner = '\n', indent:rep(depth), indent:rep(depth + 1) end
  local parts = {}
  if isList(v) then
    for i = 1, #v do parts[i] = inner .. json.encode(v[i], indent, depth + 1) end
    if #parts == 0 then return '[]' end
    return '[' .. nl .. table.concat(parts, ',' .. nl) .. nl .. pad .. ']'
  end
  local keys = {}
  for k in pairs(v) do keys[#keys + 1] = tostring(k) end
  table.sort(keys)
  for _, k in ipairs(keys) do
    local item = v[k]
    if item == nil then item = v[tonumber(k)] end
    parts[#parts + 1] = inner .. quote(k) .. (indent and ': ' or ':') .. json.encode(item, indent, depth + 1)
  end
  return '{' .. nl .. table.concat(parts, ',' .. nl) .. nl .. pad .. '}'
end

return json
