#!/usr/bin/env bash
# Builds the game for a browser: LÖVE and the core as one WebAssembly module, the game as game.love, and the page.
# Usage: ports/web/build.sh [game]. The files go to ports/web/dist. README.md has the tools and the steps.
#   (no argument)  all the steps. The first build of LÖVE takes some minutes; a later build makes only what changed.
#   game           only game.love and the page, for a change of ports/love with no change of the core.
# The script is for bash: emsdk_env.sh of the Emscripten SDK finds its folder only in bash, zsh, or ksh.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
tools=${CHROGUE_TOOLS:-${XDG_CACHE_HOME:-$HOME/.cache}/chrogue-tools}
work=${CHROGUE_WEB_WORK:-$tools/web}
emsdk=${EMSDK:-$tools/emsdk}
mkdir -p "${CHROGUE_WEB_DIST:-$here/dist}"
dist=$(cd "${CHROGUE_WEB_DIST:-$here/dist}" && pwd)

# The source of LÖVE 11.5 for Emscripten. It is the only port that uses WebAssembly exceptions, as the library of the
# core does. The commits are set, thus a change of the branch does not change the build.
megasource_url=https://github.com/alexjgriffith/megasource.git
megasource_commit=920e6113f8eb0ce54688bcfcbd150562a4aa1c96
love_url=https://github.com/alexjgriffith/love.git
love_commit=87189e63432ac07c1ee88a78e2e09e8e23a0d414

need() {
  for tool in "$@"; do
    command -v "$tool" > /dev/null 2>&1 || { echo "build.sh needs $tool. See ports/web/README.md." >&2; exit 1; }
  done
}

# Gets one commit of a repository into a folder and applies a patch to it. A folder that has the commit and the patch
# stays as it is, thus the next build makes only what changed.
checkout() {
  local dir=$1 url=$2 commit=$3 patch=$4
  local stamp="$dir/.chrogue-patch"
  if [ "$(git -C "$dir" rev-parse HEAD 2> /dev/null)" = "$commit" ] && cmp -s "$patch" "$stamp"; then return; fi
  rm -rf "$dir"
  git init -q "$dir"
  git -C "$dir" fetch -q --depth 1 "$url" "$commit"
  git -C "$dir" checkout -q FETCH_HEAD
  git -C "$dir" apply --whitespace=nowarn "$patch"
  cp "$patch" "$stamp"
}

# game.love: the files of ports/love. With CHROGUE_WEB_TESTS=1, the test scripts are also in the file.
# The page: index.html with a stamp of the three files in the addresses that it loads.
pack() {
  need bsdtar sha256sum
  if [ ! -f "$dist/love.js" ] || [ ! -f "$dist/love.wasm" ]; then
    echo "$dist has no love.js and love.wasm. Run build.sh with no argument first." >&2
    exit 1
  fi
  rm -f "$dist/game.love"
  local tests=(--exclude=test)
  if [ "${CHROGUE_WEB_TESTS:-}" = 1 ]; then tests=(); fi
  # The names in the file have no "./" before them: LÖVE looks for main.lua.
  (cd "$root/ports/love" && bsdtar -c --format zip -f "$dist/game.love" --exclude=README.md "${tests[@]}" -- *)
  local build
  build=$(cat "$dist/love.js" "$dist/love.wasm" "$dist/game.love" | sha256sum | cut -c 1-12)
  sed "s/@BUILD@/$build/g" "$here/index.html" > "$dist/index.html"
  # The server of the container is a user with no rights of its own.
  chmod a+r "$dist/index.html" "$dist/love.js" "$dist/love.wasm" "$dist/game.love"
}

engine() {
  need git cargo cmake ninja
  if [ ! -f "$emsdk/emsdk_env.sh" ]; then
    echo "build.sh did not find the Emscripten SDK at $emsdk. Set EMSDK. See ports/web/README.md." >&2
    exit 1
  fi
  # shellcheck disable=SC1091
  . "$emsdk/emsdk_env.sh" > /dev/null 2>&1
  need emcc emcmake
  echo "Emscripten: $(emcc --version | head -n 1)"

  # The core for Emscripten: target/wasm32-unknown-emscripten/release/libchrogue_core.a.
  (cd "$root/core" && cargo build --locked --release -p chrogue-embed --target wasm32-unknown-emscripten)

  # The two patches make each library static, and add the Lua module chrogue_core and a larger stack to LÖVE.
  checkout "$work/megasource" "$megasource_url" "$megasource_commit" "$here/megasource.patch"
  checkout "$work/megasource/libs/love" "$love_url" "$love_commit" "$here/love.patch"

  # Each library must use WebAssembly exceptions and the longjmp that goes with them, as the core does.
  local flags="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm"
  mkdir -p "$work/build"
  cd "$work/build"
  # LOVEJS_COMPAT: no threads. A page with threads needs headers that the server of the game does not send.
  emcmake cmake "$work/megasource" -G Ninja -DCMAKE_BUILD_TYPE=Release -DLOVE_JIT=0 -DLOVEJS_COMPAT=1 \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DLUA_BUILD_STATIC=ON -DMPG123_BUILD_STATIC=ON \
    "-DCMAKE_C_FLAGS=$flags" "-DCMAKE_CXX_FLAGS=$flags" \
    "-DCHROGUE_CORE_LIB=$root/core/target/wasm32-unknown-emscripten/release/libchrogue_core.a" \
    "-DCHROGUE_CORE_LUA=$here/chrogue_core_lua.c" "-DCHROGUE_CORE_INCLUDE=$root/core/embed/include" > configure.log
  ninja love
  cp "$work/build/love/love.js" "$work/build/love/love.wasm" "$dist/"
}

case "${1:-all}" in
  all) need bsdtar sha256sum; engine; pack ;;
  game) pack ;;
  *) echo "Usage: build.sh [game]" >&2; exit 1 ;;
esac
echo "The game for a browser is in $dist"
