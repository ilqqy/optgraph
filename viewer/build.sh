#!/usr/bin/env bash
# viewer/build.sh SRC_DIR OUT_DIR
#
# Inlines the stylesheet, the vendored fonts (base64, with their licence;
# checked against vendor/fonts/SHA256SUMS) and the app sources into
# OUT_DIR/index.html: one file, no external references.
set -euo pipefail
src=$1
out=$2
mkdir -p "$out"

app=(util data search layout vlist graph panels palette tour main)
fonts=("Geist:Geist.woff2" "Geist Mono:GeistMono.woff2")

(cd "$src/vendor/fonts" && sha256sum --quiet --strict -c SHA256SUMS) || {
  echo "build.sh: vendor/fonts do not match SHA256SUMS" >&2
  exit 1
}

for f in "${app[@]/#/$src/src/}"; do
  f=$f.js
  if grep -qi '</script' "$f"; then
    echo "build.sh: $f contains </script" >&2
    exit 1
  fi
done

while IFS= read -r line; do
  case $line in
  '/*__CSS__*/') cat "$src/style.css" ;;
  '/*__VENDOR__*/')
    printf '/*! %s\n' "$(cat "$src/vendor/VERSIONS")"
    sed 's#\*/#* /#g' "$src/vendor/fonts/OFL.txt"
    printf '*/\n'
    # Loaded with the FontFace API from these bytes (src/main.js): no
    # font-src fetch, so the page's Content-Security-Policy stays as it is.
    printf 'const OPTGRAPH_FONTS = [\n'
    for f in "${fonts[@]}"; do
      printf '  ["%s", "%s"],\n' "${f%%:*}" "$(base64 -w0 "$src/vendor/fonts/${f##*:}")"
    done
    printf '];\n'
    ;;
  '/*__APP__*/')
    printf '"use strict";\n(() => {\n'
    for f in "${app[@]}"; do
      printf '// --- %s.js\n' "$f"
      cat "$src/src/$f.js"
    done
    printf '})();\n'
    ;;
  *) printf '%s\n' "$line" ;;
  esac
done <"$src/template.html" >"$out/index.html"
