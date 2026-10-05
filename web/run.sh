#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mode=all
port=8000
open_page=1

usage() {
  cat <<'HELP'
Usage: web/run.sh [game] [--port PORT] [--no-open] [--help]

Build the game, serve it at http://127.0.0.1:8000/, and open the browser.
  game        Package the client and page only. A full build must exist first.
  --port PORT Use a port from 1 to 65535 instead of 8000.
  --no-open   Print the URL without opening the browser.
  --help      Show this help without building.
Ctrl+C stops the server.
HELP
}

while (( $# )); do
  case "$1" in
    game)
      if [[ $mode == game ]]; then
        echo "Give game only once." >&2
        exit 2
      fi
      mode=game; shift ;;
    --port)
      if [[ $# -lt 2 ]] || [[ ! $2 =~ ^[0-9]{1,5}$ ]]; then
        echo "--port needs a number from 1 to 65535." >&2
        exit 2
      fi
      port=$((10#$2))
      if (( port < 1 || port > 65535 )); then
        echo "--port needs a number from 1 to 65535." >&2
        exit 2
      fi
      shift 2 ;;
    --no-open) open_page=0; shift ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

command -v python3 > /dev/null 2>&1 || { echo "web/run.sh needs Python 3." >&2; exit 1; }
"$here/build.sh" "$mode"
dist=$(cd "${CHROGUE_WEB_DIST:-$here/dist}" && pwd)
exec python3 - "$dist" "$port" "$open_page" <<'PY'
import functools
import http.server
import sys
import threading
import webbrowser


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


dist, port, open_page = sys.argv[1:]
url = f"http://127.0.0.1:{port}/"


def open_browser():
    try:
        opened = webbrowser.open(url, new=2)
    except Exception as error:
        print(f"Browser opener failed: {error}", file=sys.stderr, flush=True)
        opened = False
    if not opened:
        print(f"Open {url} in your browser.", file=sys.stderr, flush=True)


try:
    handler = functools.partial(Handler, directory=dist)
    with http.server.ThreadingHTTPServer(("127.0.0.1", int(port)), handler) as server:
        print(f"Chrogue: {url}\nCtrl+C stops the server.", flush=True)
        if open_page == "1":
            threading.Thread(target=open_browser, daemon=True).start()
        server.serve_forever()
except OSError as error:
    print(f"Cannot serve {url}: {error}. Use --port to select another port.", file=sys.stderr)
    sys.exit(1)
except KeyboardInterrupt:
    print("\nStopped.", flush=True)
    sys.exit(130)
PY
