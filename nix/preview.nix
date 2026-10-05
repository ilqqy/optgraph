# Depth- and length-limited string preview of an arbitrary value.
#
# Values are inspected only with typeOf / attrNames / length / elemAt, and every
# node is forced through builtins.tryEval. Never calls toJSON or toString on a
# non-scalar, never reads outPath/drvPath of a derivation (only `.name`).
# Errors tryEval cannot catch (abort, missing attribute, ...) still propagate;
# callers wrap previews in a "preview" stage marker so the CLI can degrade.
#
# Internally every node yields { s = text; e = whether a throw/assert was hit; }.
let
  inherit (builtins)
    any
    attrNames
    concatStringsSep
    elemAt
    genList
    isString
    length
    match
    stringLength
    substring
    toJSON
    tryEval
    typeOf
    unsafeDiscardStringContext
    ;

  maxDepth = 3;
  maxItems = 8;
  maxString = 80;
  maxTotal = 240;

  clip = n: s: if stringLength s > n then substring 0 n s + "..." else s;

  plain = s: unsafeDiscardStringContext (clip maxString s);

  showKey = k: if match "[a-zA-Z_][a-zA-Z0-9_'-]*" k != null then k else toJSON k;

  more = n: shown: if n > shown then [ "...(${toString (n - shown)} more)" ] else [ ];

  ok = s: {
    inherit s;
    e = false;
  };
  failed = {
    s = "<error>";
    e = true;
  };

  # Join child results: open + items + close, error if any child errored.
  join = open: close: items: extra: {
    s = open + concatStringsSep " " (map (i: i.s) items ++ extra) + close;
    e = any (i: i.e) items;
  };

  go =
    depth: v:
    let
      t = tryEval (typeOf v);
    in
    if !t.success then
      failed
    else if t.value == "null" then
      ok "null"
    else if t.value == "bool" || t.value == "int" || t.value == "float" then
      ok (toJSON v)
    else if t.value == "string" then
      ok (toJSON (plain v))
    else if t.value == "path" then
      ok (plain (toString v))
    else if t.value == "lambda" then
      ok "<lambda>"
    else if t.value == "list" then
      list depth v
    else if t.value == "set" then
      set depth v
    else
      ok "<${t.value}>";

  list =
    depth: xs:
    let
      n = length xs;
      shown = if n > maxItems then maxItems else n;
    in
    if n == 0 then
      ok "[ ]"
    else if depth >= maxDepth then
      ok "[ ...(${toString n}) ]"
    else
      join "[ " " ]" (genList (i: go (depth + 1) (elemAt xs i)) shown) (more n shown);

  set =
    depth: s:
    let
      isDrv = tryEval ((s.type or null) == "derivation");
    in
    if !isDrv.success then
      failed
    else if isDrv.value then
      let
        name = tryEval (s.name or null);
      in
      if name.success && isString name.value then ok "<drv ${plain name.value}>" else failed
    else if s ? __functor then
      ok "<functor>"
    else if s ? _type then
      typed s
    else
      let
        names = attrNames s;
        n = length names;
        shown = if n > maxItems then maxItems else n;
        item =
          i:
          let
            k = elemAt names i;
            r = go (depth + 1) s.${k};
          in
          {
            s = "${showKey k} = ${r.s};";
            inherit (r) e;
          };
      in
      if n == 0 then
        ok "{ }"
      else if depth >= maxDepth then
        ok "{ ...(${toString n}) }"
      else
        join "{ " " }" (genList item shown) (more n shown);

  # literalExpression / literalMD (option defaultText) show their text; module
  # system wrappers (mkIf, mkOverride, ...) and other typed values show their
  # `_type`, e.g. `<if>`, `<override>`.
  typed =
    s:
    let
      t = tryEval s._type;
      text = tryEval s.text;
    in
    if !t.success || !isString t.value then
      failed
    else if
      (t.value == "literalExpression" || t.value == "literalMD") && text.success && isString text.value
    then
      ok (plain text.value)
    else
      ok "<${t.value}>";

  finish = r: {
    text = clip maxTotal r.s;
    failed = r.e;
  };
in
{
  inherit
    maxDepth
    maxItems
    maxString
    maxTotal
    ;

  # preview :: any -> { text :: string; failed :: bool }
  preview = v: finish (go 0 v);

  # Preview of an option's documented default: `defaultText` when present (it
  # never forces the default), otherwise the default itself.
  previewDefault =
    opt:
    if opt ? defaultText then
      let
        d = tryEval opt.defaultText;
      in
      if !d.success then
        finish failed
      else if isString d.value then
        finish (ok (unsafeDiscardStringContext d.value))
      else
        finish (go 0 d.value)
    else
      finish (go 0 opt.default);
}
