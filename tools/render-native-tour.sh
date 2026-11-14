#!/usr/bin/env bash
# tools/render-native-tour.sh [WORKDIR]
#
# Rebuilds docs/native-tour.gif: a slideshow of the native app on the demo
# graph. Builds the app (nix build .#optgraph-app) and the demo's graph.json,
# takes one screenshot per scene with tools/native-shot.sh (headless cage,
# pixman, lavapipe; run through `nix develop .#native`), driven by --opt,
# --instance and keys, then the default dev shell's ffmpeg adds a caption bar
# (drawtext, Geist from viewer/vendor/fonts), crossfades the scenes and
# encodes one GIF (one palette, 960 px wide). Screenshots and intermediate
# frames stay in WORKDIR (default /tmp/optgraph-native-tour).
#
# The output is deterministic: the screenshots are software-rendered and the
# encode is single-threaded, so two runs give byte-identical files.
set -euo pipefail
cd "$(dirname "$0")/.."

work=${1:-/tmp/optgraph-native-tour}
out=docs/native-tour.gif
hold=2.5 # seconds per scene, crossfade included
fade=0.4 # crossfade
rate=10  # GIF frame rate
width=960
mkdir -p "$work/shots" "$work/scenes"
rm -f "$work"/shots/*.png "$work"/scenes/*.png "$work"/scenes/*.txt

echo "render-native-tour: building the app and the demo graph" >&2
app=$(nix build .#optgraph-app --no-link --print-out-paths)/bin/optgraph-app
graph=$work/demo.json
nix run . -- ./demo#nixosConfigurations.demo -o "$graph" 2>&1 | tail -n 1 >&2

# name | native-shot.sh options (before --) | app arguments | caption
scenes=(
  "override||--opt networking.firewall.enable|mkForce (50) in hardening.nix beats mkDefault (1000) in dev-tools.nix"
  "mkif||--opt services.printing.enable|printing.nix sets it under mkIf false, so the option default wins"
  "merged||--opt environment.systemPackages|A list merges every winner: six modules plus 69 nixpkgs definitions"
  "instance||--instance users.users.demo --opt users.users.demo.extraGroups|The instance filter: users.users.demo, its cluster and the modules that wrote it"
  "palette|-k slash -s 400 users||/ or Ctrl+K searches options, modules and instances"
)

shot() { nix develop .#native -c tools/native-shot.sh "$@" >/dev/null; }
ff() { nix develop -c ffmpeg -hide_banner -loglevel error -y -threads 1 "$@"; }

font=viewer/vendor/fonts/Geist.woff2
mono=viewer/vendor/fonts/GeistMono.woff2
bg=0x0b0c10
bar=64
height=$((width * 1000 / 1600 + bar))
cap="fontfile=$font:fontsize=21:fontcolor=0xe6e8ee:x=(w-text_w)/2:y=h-$bar+(($bar-text_h)/2)"

i=0
for s in "${scenes[@]}"; do
  IFS='|' read -r name keys args caption <<<"$s"
  i=$((i + 1))
  echo "render-native-tour: scene $i ($name)" >&2
  png=$work/shots/$i-$name.png
  # shellcheck disable=SC2086 # app arguments are words
  if [ -n "$keys" ]; then
    shot -o "$png" -k "$keys" -- "$app" "$graph" --dark $args
  else
    shot -o "$png" -- "$app" "$graph" --dark $args
  fi
  printf '%s' "$caption" >"$work/scenes/$i.txt"
  ff -i "$png" -vf "scale=$width:-2:flags=lanczos,pad=$width:$height:0:0:color=$bg,drawtext=textfile=$work/scenes/$i.txt:$cap" \
    "$work/scenes/$i.png"
done

# End card.
i=$((i + 1))
printf 'optgraph' >"$work/scenes/end-title.txt"
printf 'Who set each NixOS option, at what priority, and why the others lost.' >"$work/scenes/end-sub.txt"
printf 'nix run github:ilqqy/optgraph#app -- .#nixosConfigurations.<host>' >"$work/scenes/end-cmd.txt"
ff -f lavfi -i "color=c=$bg:s=${width}x$height" -frames:v 1 -vf "\
drawtext=textfile=$work/scenes/end-title.txt:fontfile=$font:fontsize=64:fontcolor=0xf2f3f7:x=(w-text_w)/2:y=h/2-120,\
drawtext=textfile=$work/scenes/end-sub.txt:fontfile=$font:fontsize=22:fontcolor=0xa8adbb:x=(w-text_w)/2:y=h/2-20,\
drawtext=textfile=$work/scenes/end-cmd.txt:fontfile=$mono:fontsize=19:fontcolor=0x4ade80:box=1:boxcolor=0x16181f:boxborderw=14:x=(w-text_w)/2:y=h/2+50" \
  "$work/scenes/$i.png"

# Crossfade chain: scene k starts fading into k+1 at k*(hold-fade).
n=$i
inputs=() chain= prev=0:v
for k in $(seq 1 "$n"); do
  inputs+=(-loop 1 -framerate "$rate" -t "$hold" -i "$work/scenes/$k.png")
done
for k in $(seq 1 $((n - 1))); do
  offset=$(awk -v k="$k" -v h="$hold" -v f="$fade" 'BEGIN { printf "%.2f", k * (h - f) }')
  chain+="[$prev][$k:v]xfade=transition=fade:duration=$fade:offset=$offset[x$k];"
  prev=x$k
done
chain+="[$prev]format=rgb24,split[a][b];[a]palettegen=stats_mode=diff:max_colors=256[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
echo "render-native-tour: encoding $out" >&2
ff "${inputs[@]}" -filter_complex "$chain" -r "$rate" -fflags +bitexact "$out"
echo "render-native-tour: wrote $out ($(stat -c %s "$out") bytes)" >&2
