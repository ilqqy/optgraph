#!/usr/bin/env bash
# tools/render-tour.sh [--video [--fps N] [--scale S]] [WORKDIR]
#
# Rebuilds docs/demo.gif (README) and docs/demo.webm (site) from the viewer's
# scripted tour (viewer/src/tour.js) on the demo graph: builds the viewer and
# the demo's graph.json, captures every frame of ?tour&t=<ms> at 15 fps in
# headless Chromium (tools/tour-capture.py with the dev shell's python3, fails
# on console errors), then encodes with the dev shell's ffmpeg. Frames stay in
# WORKDIR (default /tmp/optgraph-tour). Needs a system chromium on PATH (or
# $CHROMIUM); it is not in the dev shell.
#
# --video: a smooth video instead, written to WORKDIR/demo-<fps>.mp4 and
# nothing in the repo: frames at t = i*1000/fps ms (default 60 fps), still
# 1280x720 CSS pixels but at device scale factor S (default 1.5, so
# 1920x1080), in WORKDIR/frames-video. H.264 (yuv420p, +faststart), CRF 20,
# raised until the file is under 8 MB; VP9 .webm if ffmpeg has no libx264.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
  echo "usage: $0 [--video [--fps N] [--scale S]] [WORKDIR]" >&2
  exit 1
}
video=0 fps= scale= work=/tmp/optgraph-tour
while [ $# -gt 0 ]; do
  case $1 in
  --video) video=1 ;;
  --fps) [ $# -ge 2 ] || usage && fps=$2 && shift ;;
  --scale) [ $# -ge 2 ] || usage && scale=$2 && shift ;;
  -*) usage ;;
  *) work=$1 ;;
  esac
  shift
done
if [ "$video" = 1 ]; then
  fps=${fps:-60} scale=${scale:-1.5} frames=$work/frames-video
elif [ -n "$fps$scale" ]; then
  echo "render-tour: --fps and --scale need --video (the GIF and WebM are 15 fps at scale 1)" >&2
  exit 1
else
  fps=15 scale=1 frames=$work/frames
fi
mkdir -p "$work/site"
rm -rf "$frames"

nix build .#viewer -o "$work/viewer"
install -m 644 "$work/viewer/index.html" "$work/site/index.html"
nix run . -- ./demo#nixosConfigurations.demo -o "$work/site/demo.json"
started=$(date +%s)
nix develop -c python3 -I -B tools/tour-capture.py "$work/site" "$frames" --fps "$fps" --scale "$scale"
captured=$(date +%s)

ffmpeg() { nix develop -c ffmpeg -hide_banner -loglevel error -y "$@"; }

if [ "$video" = 1 ]; then
  encoders=$(nix develop -c ffmpeg -hide_banner -encoders 2>/dev/null)
  if [[ $encoders == *" libx264 "* ]]; then
    out=$work/demo-$fps.mp4
    enc=(-c:v libx264 -preset slow -pix_fmt yuv420p -movflags +faststart)
  else
    out=$work/demo-$fps.webm
    enc=(-c:v libvpx-vp9 -b:v 0 -pix_fmt yuv420p -row-mt 1)
    echo "render-tour: ffmpeg has no libx264, writing VP9 $out" >&2
  fi
  for crf in 20 23 26 29 32; do
    ffmpeg -framerate "$fps" -i "$frames/%04d.png" "${enc[@]}" -crf "$crf" -an -fflags +bitexact "$out"
    size=$(stat -c %s "$out")
    echo "render-tour: $out crf $crf: $size bytes"
    [ "$size" -lt 8000000 ] && break
  done
  [ "$size" -lt 8000000 ] || { echo "render-tour: $out is still 8 MB or more" >&2; exit 1; }
  echo "render-tour: capture $((captured - started)) s, encode $(($(date +%s) - captured)) s"
  exit 0
fi

# GIF: one palette for the whole tour, only the changed rectangle per frame.
# Kept under 3 MB: 960 px wide at 15 fps first, then fewer frames or pixels.
for try in 960:15 960:12 800:12 720:10; do
  width=${try%:*} rate=${try#*:}
  ffmpeg -framerate "$fps" -i "$frames/%04d.png" \
    -vf "fps=$rate,scale=$width:-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
    -loop 0 docs/demo.gif
  size=$(stat -c %s docs/demo.gif)
  echo "render-tour: docs/demo.gif ${width}px ${rate} fps: $size bytes"
  [ "$size" -lt 3000000 ] && break
done
[ "$size" -lt 3000000 ] || { echo "render-tour: docs/demo.gif is still 3 MB or more" >&2; exit 1; }

ffmpeg -framerate "$fps" -i "$frames/%04d.png" \
  -c:v libvpx-vp9 -crf 33 -b:v 0 -pix_fmt yuv420p -row-mt 1 -an -fflags +bitexact docs/demo.webm
echo "render-tour: docs/demo.webm 1280x720 $fps fps: $(stat -c %s docs/demo.webm) bytes"
