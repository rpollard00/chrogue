#!/bin/sh
# Runs the scripts of test/ against the real game and a real core. Each run opens a window for some seconds.
# Usage: test/run.sh [output folder]. The screenshots, the dumps and the logs go to the output folder.
# Build the core first: cargo build --release in ../core.
# CHROGUE_NO_AUTH=1 adds --no-auth to each run, for a core that does not check the token of a connection.
#
# The run fails if a game stops with an error, if the core refused a command that a script did not expect (the client
# writes "The core refused" on stderr, and a script stops at the refusal), or if a core of this game continues after the tests.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
love=${LOVE:-$HOME/.cache/chrogue-tools/love.AppImage}
out=${1:-/tmp/chrogue-love4/run}
core=${CHROGUE_CORE:-$here/../core/target/release/chrogue-core}
auth=
if [ "${CHROGUE_NO_AUTH:-}" = 1 ]; then auth=--no-auth; fi
mkdir -p "$out"
export CHROGUE_OUT="$out"
log="$out/run.log"
: > "$log"
failed=0

run() {
  echo "== $*" >> "$log"
  # $auth is empty or one word, thus it has no quotes.
  # shellcheck disable=SC2086
  if ! timeout 240 "$love" "$here" $auth "$@" >> "$log" 2>&1; then
    echo "The run failed: $*" >> "$log"
    failed=1
  fi
}

run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/flow.lua"
run --size 1920x1080 --seed 7 --debug --no-save --script "$here/test/flow.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/showcase.lua"
run --size 1920x1080 --seed 7 --debug --no-save --script "$here/test/showcase.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/floor8.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/input.lua"
run --size 1920x1080 --seed 7 --debug --no-save --script "$here/test/input.lua"
run --size 1280x720 --seed 7 --debug --no-save --script "$here/test/battle.lua"
run --size 1440x900 --seed 7 --debug --no-save --script "$here/test/auras.lua"
run --size 1440x900 --seed 7 --debug --no-save --script "$here/test/denied.lua"
# The debug menu. The run has no --debug: the core accepts the debug commands unless the game gets --no-debug.
run --size 1280x720 --seed 7 --no-save --script "$here/test/debug.lua"
run --size 1280x720 --seed 7 --no-save --script "$here/test/relics.lua"
run --size 1280x720 --no-save --script "$here/test/icons.lua"
# The core in the process of the game: the same session with no socket and no core program. Then a saved run: the
# first game starts a battle and quits, and a second game on the same save folder continues the run.
run --embed --size 1280x720 --seed 7 --debug --no-save --script "$here/test/flow.lua"
rm -rf "$out/save-embed"
run --embed --seed 7 --save-dir "$out/save-embed" --script "$here/test/saved-a.lua"
run --embed --seed 7 --save-dir "$out/save-embed" --script "$here/test/saved-b.lua"

# The notices about the saved data: the save folder cannot take a file.
for size in 1280x720 1920x1080; do
  [ -d "$out/read-only" ] && chmod u+w "$out/read-only"
  rm -rf "$out/read-only"
  mkdir -p "$out/read-only"
  # The core must be able to take its lock file, thus the file exists before the folder closes.
  touch "$out/read-only/chrogue-core.lock"
  chmod 555 "$out/read-only"
  run --size "$size" --seed 7 --save-dir "$out/read-only" --script "$here/test/saves.lua"
done
chmod u+w "$out/read-only"

# The reconnection: the first game quits in a battle, the core continues, and a second game connects to it with the
# address and the token that the first game printed.
run --keep-alive --seed 7 --no-save --script "$here/test/reconnect-a.lua"
line=$(grep 'The core continues at' "$log" | tail -n 1)
address=$(echo "$line" | grep -o -- '--connect 127.0.0.1:[0-9]*' | grep -o '127.0.0.1:[0-9]*')
token=$(echo "$line" | grep -o 'CHROGUE_TOKEN=[0-9a-f]*' | cut -d= -f2)
CHROGUE_TOKEN=$token run --connect "$address" --script "$here/test/reconnect-b.lua"

# The lost connection: the core stops, and the game starts a new one with the same save folder.
rm -rf "$out/save"
mkdir -p "$out/save"
run --save-dir "$out/save" --seed 7 --script "$here/test/lost.lua"

run --size 1920x1080 --novsync --seed 7 --debug --no-save --script "$here/test/fps.lua"

grep -E '^(ok|fps|enemy moves|expected)' "$log"
if grep -n -i -E 'error|traceback|refused' "$log"; then
  echo "The log has errors or refusals that a script did not expect: $log"
  failed=1
fi
if grep -n 'The run failed' "$log"; then failed=1; fi
# Only a core of this game: other programs can also run a chrogue-core.
real=$(readlink -f "$core")
for pid in $(pgrep -x chrogue-core); do
  if [ "$(readlink -f "/proc/$pid/exe" 2>/dev/null)" = "$real" ]; then
    echo "A core of this game is still running: $(tr '\0' ' ' < "/proc/$pid/cmdline")"
    failed=1
  fi
done
if [ "$failed" = 1 ]; then
  echo "The tests failed. See $log"
  exit 1
fi
echo "No errors in $log"
