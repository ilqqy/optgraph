#!/usr/bin/env bash
# viewer/build.sh SRC_DIR OUT_DIR
#
# Inlines the stylesheet, the vendored d3 builds (with their licences) and the
# app sources into OUT_DIR/index.html: one file, no external references.
set -euo pipefail
src=$1
out=$2
mkdir -p "$out"

vendor=(d3-dispatch d3-quadtree d3-timer d3-force) # dependency order
app=(util data search vlist graph panels tour main)

for f in "${vendor[@]/#/$src/vendor/}" "${app[@]/#/$src/src/}"; do
  case $f in */vendor/*) f=$f.min.js ;; *) f=$f.js ;; esac
  if grep -qi '</script' "$f"; then
    echo "build.sh: $f contains </script" >&2
    exit 1
  fi
done

while IFS= read -r line; do
  case $line in
  '/*__CSS__*/') cat "$src/style.css" ;;
  '/*__VENDOR__*/')
    for v in "${vendor[@]}"; do
      printf '/*! %s, %s\n' "$v" "$(grep "^$v " "$src/vendor/VERSIONS" | cut -d' ' -f2)"
      sed 's#\*/#* /#g' "$src/vendor/$v.LICENSE"
      printf '*/\n'
      cat "$src/vendor/$v.min.js"
      printf '\n'
    done
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
