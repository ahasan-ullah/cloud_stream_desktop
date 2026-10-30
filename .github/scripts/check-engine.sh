#!/usr/bin/env bash
# Starts the engine with the Java runtime packaged into the app and checks that it answers,
# so a broken runtime fails the build here instead of on a user's machine.
set -euo pipefail

build_dir="${BUILD_DIR:-app/src-tauri/target/release}"
work_dir="$(mktemp -d)"

case "${RUNNER_OS:-}" in
  macOS)
    apps=("$build_dir"/bundle/macos/*.app)
    resources="${apps[0]}/Contents/Resources"
    ;;
  Linux)
    # The .deb is what users install, so test the files as they end up on disk.
    debs=("$build_dir"/bundle/deb/*.deb)
    dpkg-deb -x "${debs[0]}" "$work_dir/deb"
    java_path="$(find "$work_dir/deb" -path '*/jre/bin/java' | head -n 1)"
    resources="$(dirname "$(dirname "$(dirname "$java_path")")")"
    ;;
  *)
    resources="$build_dir"
    ;;
esac

java="$resources/jre/bin/java"
[ -x "$java" ] || java="$java.exe"

export CS_ENGINE_DATA="$work_dir/data"
export CS_ENGINE_TOKEN=ci

log="$work_dir/engine.log"
"$java" -XX:+UseSerialGC -Xmx256m -jar "$resources/engine/engine.jar" >"$log" 2>&1 &
engine=$!
trap 'kill "$engine" 2>/dev/null || true' EXIT

for _ in $(seq 1 60); do
  grep -q ENGINE_READY "$log" && break
  kill -0 "$engine" 2>/dev/null || break
  sleep 1
done

if ! grep -q ENGINE_READY "$log"; then
  echo "::error::The packaged engine did not start"
  cat "$log"
  exit 1
fi

port="$(sed -n 's/.*ENGINE_READY port=\([0-9]*\).*/\1/p' "$log")"
curl -fsS "http://127.0.0.1:$port/health"
echo
echo "Engine answered on port $port"
