#!/bin/sh
# Drives the real port with the scripts of test/app/ and saves screenshots and state files.
# Each run opens a window for some seconds. Usage: test/app.sh [output folder]
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
love=${LOVE:-$HOME/.cache/chrogue-tools/love.AppImage}
export CHROGUE_OUT=${1:-/tmp/chrogue-ports/love}
mkdir -p "$CHROGUE_OUT"
log="$CHROGUE_OUT/app.log"
: > "$log"

run() {
  echo "== $*" >> "$log"
  timeout 120 "$love" "$here" "$@" >> "$log" 2>&1
}

run --seed 7 --script "$here/test/app/play.lua"
run --demo mate --script "$here/test/app/mate.lua"
run --demo promo --seed 7 --script "$here/test/app/promo.lua"
run --demo check --seed 7 --script "$here/test/app/check.lua"
CHROGUE_NAME=defeat run --demo defeat --script "$here/test/app/ends.lua"
CHROGUE_NAME=draw run --demo draw --script "$here/test/app/ends.lua"
CHROGUE_NAME=start-1440x900 run --size 1440x900 --script "$here/test/app/start.lua"
CHROGUE_NAME=start-1920x1080 run --size 1920x1080 --script "$here/test/app/start.lua"

luajit "$here/test/check-app.lua" "$CHROGUE_OUT"
if grep -n -i -E 'error|traceback' "$log"; then
  echo "The log has errors: $log"
  exit 1
fi
echo "No errors in $log"
