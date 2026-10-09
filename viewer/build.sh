#!/usr/bin/env bash
# viewer/build.sh SRC_DIR OUT_DIR
#
# Inlines the stylesheet and the app sources into OUT_DIR/index.html: one
# file, no external references.
set -euo pipefail
src=$1
out=$2
mkdir -p "$out"

app=(util data search layout vlist graph panels tour main)

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
  '/*__VENDOR__*/') ;;
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
