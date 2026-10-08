# optgraph-embed-html VIEWER_HTML GRAPH_JSON OUT_HTML
#
# Writes OUT_HTML: the viewer with GRAPH_JSON in its
# <script type="application/json" id="optgraph-data"> element. Every `<` in
# the JSON (which can only occur inside strings) becomes <, so the data
# can never close the script element. Atomic: written to a temp file first.

if [ $# -ne 3 ]; then
  echo "usage: optgraph-embed-html VIEWER_HTML GRAPH_JSON OUT_HTML" >&2
  exit 1
fi
viewer=$1
json=$2
out=$3
prefix='<script type="application/json" id="optgraph-data">'
marker="$prefix/*OPTGRAPH_DATA*/null</script>"

offset=$(grep -bo -F "$marker" "$viewer" | head -n 1 | cut -d: -f1)
if [ -z "$offset" ]; then
  echo "optgraph-embed-html: no data placeholder in $viewer" >&2
  exit 1
fi

tmp="$out.tmp.$$"
trap 'rm -f "$tmp"' EXIT
{
  head -c "$((offset + ${#prefix}))" "$viewer"
  jq -c . "$json" | sed 's/</\\u003c/g'
  printf '</script>'
  tail -c +"$((offset + ${#marker} + 1))" "$viewer"
} >"$tmp"
mv -f "$tmp" "$out"
trap - EXIT
