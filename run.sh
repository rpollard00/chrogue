#!/bin/sh
# Builds the core, then starts the game. The game starts the core.
# Usage: ./run.sh [options of the game]. client/README.md has the options, for example --size 1440x900.
# LOVE gives the path of LÖVE. Without it, the script uses love from the PATH, then ~/.cache/chrogue-tools/love.AppImage.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
love=${LOVE:-}
if [ -z "$love" ]; then
  love=$(command -v love) || love=$HOME/.cache/chrogue-tools/love.AppImage
fi
if ! command -v "$love" > /dev/null; then
  echo "The script did not find LÖVE at $love. Install LÖVE 11.5, or set LOVE to its path." >&2
  exit 1
fi
(cd "$here/core" && cargo build --release)
exec "$love" "$here/client" "$@"
