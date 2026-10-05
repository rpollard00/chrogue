#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
name=balance
open_page=1
args=()

usage() {
  cat <<'HELP'
Usage: scripts/balance.sh [--name NAME] [--no-open] [--help] [options of balance]

Play the battles of a sweep, make the HTML report, and open it in the browser.
The files are out/NAME.json and out/NAME.html.
  --name NAME  Use this name for the two files instead of balance.
  --no-open    Print the path of the report without opening the browser.
  --help       Show this help and the options of balance.
The other options go to balance, for example:
  scripts/balance.sh --player 2-4
  scripts/balance.sh --name pairs --floor 4,6,8 --relics none,each,pairs
core/README.md describes the options of balance ("Measurements of relics and armies").
HELP
}

while (( $# )); do
  case "$1" in
    --name)
      if [[ $# -lt 2 ]] || [[ ! $2 =~ ^[A-Za-z0-9._-]+$ ]]; then
        echo "--name needs a name of letters, numbers, '.', '_', and '-'." >&2
        exit 2
      fi
      name=$2; shift 2 ;;
    --no-open) open_page=0; shift ;;
    --out)
      echo "The script sets --out. Use --name NAME for out/NAME.json." >&2
      exit 2 ;;
    --help|-h)
      usage
      echo
      (cd "$here/core" && cargo run --release --quiet --bin balance -- --help 2>&1 | tail -n +2) || true
      exit 0 ;;
    *) args+=("$1"); shift ;;
  esac
done

if ! command -v bun > /dev/null; then
  echo "The script did not find bun. Install Bun, then run the script again." >&2
  exit 1
fi

mkdir -p "$here/out"
data=$here/out/$name.json
page=$here/out/$name.html

(cd "$here/core" && cargo build --release --bin balance)
"$here/core/target/release/balance" "${args[@]}" --out "$data"

cd "$here/core/tools/report"
if [[ ! -d node_modules ]]; then
  bun install
fi
bun run report "$data" "$page"

echo "The report is $page"
if (( open_page )); then
  if command -v xdg-open > /dev/null; then
    xdg-open "$page" > /dev/null 2>&1 &
  elif command -v open > /dev/null; then
    open "$page"
  fi
fi
