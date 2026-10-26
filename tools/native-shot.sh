#!/usr/bin/env bash
# Screenshot of the native app (or any Wayland client) without a display:
# runs it in cage (wlroots kiosk compositor) on a headless output with
# software rendering (pixman for the compositor, Mesa's lavapipe for the
# app's Vulkan), then captures the output with grim.
#
#   tools/native-shot.sh [-o out.png] [-s 1600x1000] [-w secs] [-k keys] [-x cmd] -- command [args...]
#
# -k types keys into the app before the capture, with wtype (the virtual
# keyboard protocol): a space-separated list of wtype arguments, e.g.
# "-k slash -s 300 fire" (slash key, wait 300 ms, type "fire"); see wtype(1).
# -x runs a shell command in the session after that, e.g. pointer moves and
# clicks with wlrctl: -x "wlrctl pointer move 700 400; wlrctl pointer click".
#
# Run it from the native dev shell (`nix develop .#native`): cage, grim,
# wlr-randr and $OPTGRAPH_LAVAPIPE_ICD come from there. The default output is
# /tmp/optgraph-native/shot.png. The app fills the output (cage maximises it).
set -euo pipefail

out=/tmp/optgraph-native/shot.png
mode=1600x1000
wait_s=4
keys=
cmd=
while [ $# -gt 0 ]; do
  case $1 in
    -o) out=$2; shift 2 ;;
    -s) mode=$2; shift 2 ;;
    -w) wait_s=$2; shift 2 ;;
    -k) keys=$2; shift 2 ;;
    -x) cmd=$2; shift 2 ;;
    --) shift; break ;;
    *) break ;;
  esac
done
[ $# -gt 0 ] || { echo "usage: $0 [-o out.png] [-s WxH] [-w secs] -- command [args...]" >&2; exit 2; }
for t in cage grim wlr-randr wtype; do
  command -v "$t" >/dev/null || { echo "native-shot: $t not found; run from 'nix develop .#native'" >&2; exit 2; }
done
[ -n "${OPTGRAPH_LAVAPIPE_ICD:-}" ] || { echo "native-shot: OPTGRAPH_LAVAPIPE_ICD unset; run from 'nix develop .#native'" >&2; exit 2; }

mkdir -p "$(dirname "$out")" /tmp/optgraph-native
run=$(mktemp -d /tmp/optgraph-native/run.XXXXXX)
chmod 700 "$run"
log=$run/cage.log
cage_pid=
cleanup() {
  [ -n "$cage_pid" ] && kill "$cage_pid" 2>/dev/null && wait "$cage_pid" 2>/dev/null
  rm -rf "$run"
}
trap cleanup EXIT

# A private runtime dir: the compositor's socket can't clash with a live
# session, and the app can't reach it either. No session D-Bus either, so
# portals (file dialogs, the colour scheme) of a live desktop stay out.
export XDG_RUNTIME_DIR=$run
export DBUS_SESSION_BUS_ADDRESS=unix:path=$run/no-bus
unset WAYLAND_DISPLAY DISPLAY
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1
export VK_DRIVER_FILES=$OPTGRAPH_LAVAPIPE_ICD VK_ICD_FILENAMES=$OPTGRAPH_LAVAPIPE_ICD

cage -- "$@" >"$log" 2>&1 &
cage_pid=$!

for _ in $(seq 100); do
  [ -S "$run/wayland-0" ] && break
  kill -0 "$cage_pid" 2>/dev/null || { echo "native-shot: cage exited" >&2; tail -n 20 "$log" >&2; exit 1; }
  sleep 0.1
done
[ -S "$run/wayland-0" ] || { echo "native-shot: no compositor socket" >&2; tail -n 20 "$log" >&2; exit 1; }
export WAYLAND_DISPLAY=wayland-0
output=$(wlr-randr | awk 'NR==1 {print $1}')
wlr-randr --output "$output" --custom-mode "$mode" >/dev/null

sleep "$wait_s"
if [ -n "$keys" ]; then
  # shellcheck disable=SC2086 # word splitting is the point: wtype arguments
  wtype -s 300 $keys # the delay lets the client take the new keymap first
  sleep 1
fi
if [ -n "$cmd" ]; then
  bash -c "$cmd"
  sleep 1
fi
kill -0 "$cage_pid" 2>/dev/null || { echo "native-shot: the app exited" >&2; tail -n 20 "$log" >&2; exit 1; }
grim "$out"
echo "$out"
if [ -n "${NATIVE_SHOT_LOG:-}" ]; then cp "$log" "$NATIVE_SHOT_LOG"; fi
