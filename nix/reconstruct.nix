# Raw definitions of options, including losing priorities and mkIf-false ones,
# recovered by walking each module's `config` alongside the option tree.
#
# Mirrors pushDownProperties / dischargeProperties / filterOverrides'
# (nixpkgs@c59305b:lib/modules.nix:1367-1474) but keeps what they drop.
# See docs/module-system-notes.md topic 3.
#
# Laziness contract:
#  - the walk forces module configs only along option paths in the trie (the
#    attrsets and mkIf/mkMerge/mkOverride wrappers the module system forces too);
#  - mkIf conditions are forced (under tryEval) only by `classify`;
#  - a definition value is forced to WHNF (to read `_type`) only when every
#    condition above it is true, and an mkIf's content only if its condition is;
#  - values themselves are never forced here (`content` stays a thunk).
# The walk stops at option boundaries: submodule-typed and freeform options
# are leaves, their definitions are listed as a whole.
{ lib }:
let
  inherit (builtins)
    any
    attrNames
    concatMap
    groupBy
    head
    intersectAttrs
    isAttrs
    isBool
    tail
    tryEval
    ;

  # Re-apply ancestor wrappers around a leaf, outermost first, like pushDownProperties.
  wrap =
    layers: v:
    lib.foldr (
      l: acc: if l ? condition then lib.mkIf l.condition acc else lib.mkOverride l.priority acc
    ) v layers;

  # Nested attrset of option paths; `true` marks an option. Built by grouping on
  # the first component (a foldl' of recursiveUpdate overflows the stack with
  # thousands of locs under one prefix). Options never nest, so a group whose
  # rest is empty is an option.
  trieOf =
    locs:
    lib.mapAttrs (
      _: group:
      let
        rest = map tail group;
      in
      if any (r: r == [ ]) rest then true else trieOf rest
    ) (groupBy head locs);

  # walk :: { options, file, module, trie } -> config -> [ { loc, file, module, raw } ]
  walk =
    {
      options,
      file,
      module,
      trie,
    }:
    let
      go =
        loc: opts: layers: trie: cfg:
        if !isAttrs cfg then
          [ ]
        else if cfg._type or null == "if" then
          go loc opts (layers ++ [ { inherit (cfg) condition; } ]) trie cfg.content
        else if cfg._type or null == "merge" then
          concatMap (go loc opts layers trie) cfg.contents
        else if cfg._type or null == "override" then
          go loc opts (layers ++ [ { inherit (cfg) priority; } ]) trie cfg.content
        else
          concatMap (
            name:
            let
              o = opts.${name};
              loc' = loc ++ [ name ];
            in
            if loc == [ ] && name == "_module" then
              [ ]
            else if lib.isOption o then
              [
                {
                  loc = loc';
                  inherit file module;
                  raw = wrap layers cfg.${name};
                }
              ]
            else
              go loc' o layers (if trie == null then null else trie.${name}) cfg.${name}
          ) (attrNames (intersectAttrs opts (if trie == null then cfg else intersectAttrs trie cfg)));
    in
    go [ ] options [ ] trie;

  # classify :: raw definition -> [ { file, conditions, priority, probeFailed, content } ]
  # One raw definition can yield several (mkMerge). `conditions` is tri-state
  # per enclosing mkIf: true / false / null (condition threw or is not a bool).
  classify =
    file: raw:
    let
      tryBool =
        c:
        let
          r = tryEval c;
        in
        if r.success && isBool r.value then r.value else null;

      discharge =
        conds: f: v:
        let
          ty = tryEval (v._type or null); # forces v to WHNF
        in
        if !ty.success then
          [
            {
              inherit conds;
              file = f;
              known = false;
              probeFailed = true;
              value = v;
            }
          ]
        else if ty.value == "if" then
          let
            c = tryBool v.condition;
          in
          if c == true then
            discharge (conds ++ [ true ]) f v.content
          else
            # false or unknown: the module system would not force the content either
            [
              {
                conds = conds ++ [ c ];
                file = f;
                known = false;
                probeFailed = false;
                value = v.content;
              }
            ]
        else if ty.value == "merge" then
          concatMap (discharge conds f) v.contents
        else if ty.value == "definition" then
          [
            {
              inherit conds;
              inherit (v) file value;
              known = true;
              probeFailed = false;
            }
          ]
        else
          [
            {
              inherit conds;
              file = f;
              value = v;
              known = true;
              probeFailed = false;
            }
          ];
    in
    map (
      d:
      let
        # For known defs `value` is already WHNF: an override wrapper is visible
        # without forcing its content. (A mkDefinition value is probed here.)
        ov = if d.known then tryEval (d.value._type or null == "override") else { success = false; };
        isOv = ov.success && ov.value;
      in
      {
        inherit (d) file;
        conditions = d.conds;
        probeFailed = d.probeFailed || (d.known && !ov.success);
        priority =
          if !d.known || !ov.success then
            null
          else if isOv then
            d.value.priority
          else
            lib.modules.defaultOverridePriority or 100;
        content = if isOv then d.value.content else d.value;
      }
    ) (discharge [ ] file raw);
in
{
  inherit
    classify
    trieOf
    walk
    wrap
    ;
}
