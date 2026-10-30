#!/usr/bin/env bash
# Installs the app, opens it and checks that it starts its engine. Finding and launching the engine
# only happens in a real copy of the app, so a broken install fails the build here and not for users.
set -euo pipefail

bundle="${BUNDLE_DIR:-app/src-tauri/target/release/bundle}"
work_dir="$(mktemp -d)"

# The app uses this folder instead of its real one, so the check runs with no plugins installed.
export CS_ENGINE_DATA="$work_dir/data"
log="$CS_ENGINE_DATA/engine.log"

if [ -n "${APP_BINARY:-}" ]; then
  run=("$APP_BINARY")
else
  case "${RUNNER_OS:-}" in
    Windows)
      installers=("$bundle"/nsis/*.exe)
      MSYS_NO_PATHCONV=1 "${installers[0]}" /S
      run=("$LOCALAPPDATA/CloudStream Desktop/cloudstream-desktop.exe")
      ;;
    macOS)
      apps=("$bundle"/macos/*.app)
      run=("${apps[0]}/Contents/MacOS/cloudstream-desktop")
      ;;
    Linux)
      debs=("$bundle"/deb/*.deb)
      sudo apt-get install -y "$PWD/${debs[0]}"
      run=(xvfb-run -a cloudstream-desktop)
      ;;
    *)
      echo "Set APP_BINARY to the app to check"
      exit 1
      ;;
  esac
fi

"${run[@]}" >"$work_dir/app.log" 2>&1 &
app=$!
trap 'kill "$app" 2>/dev/null || true' EXIT

for _ in $(seq 1 90); do
  grep -qs ENGINE_READY "$log" && break
  kill -0 "$app" 2>/dev/null || break
  sleep 1
done

if ! grep -qs ENGINE_READY "$log"; then
  output="$(cat "$log" "$work_dir/app.log" 2>/dev/null | tail -n 30 | awk '{ printf "%s%%0A", $0 }')"
  echo "::error title=The installed app did not start its engine::$output"
  exit 1
fi

port="$(sed -n 's/.*ENGINE_READY port=\([0-9]*\).*/\1/p' "$log")"
curl -fsS "http://127.0.0.1:$port/health"
echo
echo "The app started its engine on port $port"
