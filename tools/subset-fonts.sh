#!/usr/bin/env bash
# tools/subset-fonts.sh
#
# Regenerates the viewer's embedded fonts in viewer/vendor/fonts/ from the
# geist-font package of this flake's nixpkgs (Geist and Geist Mono, SIL Open
# Font License 1.1, licence from the release archive): the variable fonts,
# weights cut to 400-700, subset to Latin-1 plus a little punctuation,
# hinting dropped, as WOFF2. Then pins them in SHA256SUMS, which
# viewer/build.sh checks. Needs nix (fonttools and brotli come from nixpkgs).
set -euo pipefail
cd "$(dirname "$0")/.."

out=viewer/vendor/fonts
build() { # build EXPR: EXPR over `pkgs` (this flake's nixpkgs)
  nix build --no-link --print-out-paths --impure --expr \
    "let pkgs = (builtins.getFlake \"git+file://$PWD\").inputs.nixpkgs.legacyPackages.\${builtins.currentSystem}; in $1"
}
fonts=$(build pkgs.geist-font)/share/fonts/truetype
src=$(build "builtins.head pkgs.geist-font.srcs") # release archive, unpacked
python=$(build "pkgs.python3.withPackages (p: [ p.fonttools p.brotli ])")

# Latin-1, dashes, quotes, bullet, ellipsis, arrows, minus, check mark.
unicodes='U+0020-007E,U+00A0-00FF,U+2013-2014,U+2018-201A,U+201C-201E,U+2022,U+2026,U+203A,U+2190-2193,U+21B5,U+2212,U+2713'
export SOURCE_DATE_EPOCH=0 # fonttools stamps head.modified with it: reproducible output
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for pair in "Geist[wght]:Geist" "GeistMono[wght]:GeistMono"; do
  in=${pair%%:*} name=${pair##*:}
  "$python/bin/fonttools" varLib.instancer "$fonts/$in.ttf" wght=400:700 -o "$work/$name.ttf" -q
  "$python/bin/pyftsubset" "$work/$name.ttf" --unicodes="$unicodes" --flavor=woff2 \
    --layout-features='kern,liga,calt,tnum,zero' --no-hinting --desubroutinize --output-file="$out/$name.woff2"
done
install -m 644 "$src/OFL.txt" "$out/OFL.txt"
(cd "$out" && sha256sum Geist.woff2 GeistMono.woff2 OFL.txt >SHA256SUMS)
ls -l "$out"
