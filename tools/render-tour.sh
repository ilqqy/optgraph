#!/usr/bin/env bash
# tools/render-tour.sh [WORKDIR]
#
# Rebuilds docs/demo.gif (README) and docs/demo.webm (site) from the viewer's
# scripted tour (viewer/src/tour.js) on the demo graph: builds the viewer and
# the demo's graph.json, captures every frame of ?tour&t=<ms> at 15 fps in
# headless Chromium (tools/tour-capture.py, fails on console errors), then
# encodes with the dev shell's ffmpeg. Frames stay in WORKDIR (default
# /tmp/optgraph-tour). Needs chromium (or $CHROMIUM) and python3 on PATH.
set -euo pipefail
cd "$(dirname "$0")/.."

work=${1:-/tmp/optgraph-tour}
fps=15
mkdir -p "$work/site"
rm -rf "$work/frames"

nix build .#viewer -o "$work/viewer"
install -m 644 "$work/viewer/index.html" "$work/site/index.html"
nix run . -- ./demo#nixosConfigurations.demo -o "$work/site/demo.json"
python3 -I tools/tour-capture.py "$work/site" "$work/frames" --fps "$fps"

ffmpeg() { nix develop -c ffmpeg -hide_banner -loglevel error -y "$@"; }

# GIF: one palette for the whole tour, only the changed rectangle per frame.
# Kept under 5 MB: 960 px wide at 15 fps first, then fewer frames or pixels.
for try in 960:15 960:12 800:12 720:10; do
  width=${try%:*} rate=${try#*:}
  ffmpeg -framerate "$fps" -i "$work/frames/%04d.png" \
    -vf "fps=$rate,scale=$width:-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
    -loop 0 docs/demo.gif
  size=$(stat -c %s docs/demo.gif)
  echo "render-tour: docs/demo.gif ${width}px ${rate} fps: $size bytes"
  [ "$size" -lt 5000000 ] && break
done
[ "$size" -lt 5000000 ] || { echo "render-tour: docs/demo.gif is still 5 MB or more" >&2; exit 1; }

ffmpeg -framerate "$fps" -i "$work/frames/%04d.png" \
  -c:v libvpx-vp9 -crf 33 -b:v 0 -pix_fmt yuv420p -row-mt 1 -an -fflags +bitexact docs/demo.webm
echo "render-tour: docs/demo.webm 1280x720 $fps fps: $(stat -c %s docs/demo.webm) bytes"
