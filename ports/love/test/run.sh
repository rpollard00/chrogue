#!/bin/sh
# Runs the scripts of test/ against the real game and a real core. Each run opens a window for some seconds.
# Usage: test/run.sh [output folder]. The screenshots, the dumps and the logs go to the output folder.
# Build the core first: cargo build --release in ../../core.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
love=${LOVE:-$HOME/.cache/chrogue-tools/love.AppImage}
out=${1:-/tmp/chrogue-love4/run}
mkdir -p "$out"
export CHROGUE_OUT="$out"
log="$out/run.log"
: > "$log"

run() {
  echo "== $*" >> "$log"
  timeout 240 "$love" "$here" "$@" >> "$log" 2>&1
}

run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/flow.lua"
run --size 1920x1080 --seed 7 --debug --no-save --script "$here/test/flow.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/showcase.lua"
run --size 1920x1080 --seed 7 --debug --no-save --script "$here/test/showcase.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/floor8.lua"

# The reconnection: the first game quits in a battle, the core continues, and a second game connects to it.
run --keep-alive --seed 7 --no-save --script "$here/test/reconnect-a.lua"
address=$(grep -o 'core continues at 127.0.0.1:[0-9]*' "$log" | tail -n 1 | grep -o '127.0.0.1:[0-9]*')
run --connect "$address" --script "$here/test/reconnect-b.lua"

# The lost connection: the core stops, and the game starts a new one with the same save folder.
rm -rf "$out/save"
mkdir -p "$out/save"
run --save-dir "$out/save" --seed 7 --script "$here/test/lost.lua"

run --size 1920x1080 --novsync --seed 7 --debug --no-save --script "$here/test/fps.lua"

grep -E '^(ok|fps|enemy moves)' "$log"
if grep -n -i -E 'error|traceback' "$log"; then
  echo "The log has errors: $log"
  exit 1
fi
if pgrep -x chrogue-core > /dev/null; then
  echo "A core is still running:"
  pgrep -ax chrogue-core
  exit 1
fi
echo "No errors in $log"
