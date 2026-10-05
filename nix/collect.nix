# Re-collects the module tree of a finished evaluation and aligns its top level
# with the module lists eval-config.nix exposes in `_module.args`.
#
# Mirrors collectModules / unifyModuleSyntax / applyModuleArgs
# (nixpkgs@c59305b:lib/modules.nix:416-756). The originals are private and warn
# on external use, so they are re-implemented; see docs/module-system-notes.md
# topic 3. Every module load is wrapped in a "module" marker and tryEval.
{
  lib,
  mark,
}:
{
  sys,
  excludeModules ? [ ],
}:
let
  inherit (builtins)
    all
    any
    attrNames
    concatMap
    elem
    elemAt
    filter
    genericClosure
    head
    isAttrs
    isList
    isPath
    isString
    length
    listToAttrs
    seq
    tryEval
    unsafeGetAttrPos
    ;

  # Keys, files and slots are used as attribute names: no string context
  # (keys of `"${input}/module.nix"` imports carry store-path context).
  noCtx = builtins.unsafeDiscardStringContext;

  specialArgs = sys._module.specialArgs;
  modulesPath = specialArgs.modulesPath or "";

  args = {
    inherit lib specialArgs;
    inherit (sys) options;
    # `sys.config` has `_module` removed (lib/modules.nix:404); modules may read config._module.args.
    config = sys.config // {
      inherit (sys) _module;
    };
    _class = sys.class or null;
    _prefix = [ ];
  }
  // specialArgs;

  applyArgs =
    f:
    f (
      args
      // lib.mapAttrs (name: _: args.${name} or args.config._module.args.${name}) (lib.functionArgs f)
    );

  # Position of the first attribute of a raw (already applied) module: the best
  # available location of anonymous modules, whose `_file` is only a fallback.
  firstPos =
    m:
    let
      names = attrNames m;
    in
    if names == [ ] then null else unsafeGetAttrPos (head names) m;

  unify =
    file: key: m:
    let
      addMeta =
        c:
        if m ? meta then
          lib.mkMerge [
            c
            { meta = m.meta; }
          ]
        else
          c;
      addFreeform =
        c:
        if m ? freeformType then
          lib.mkMerge [
            c
            { _module.freeformType = m.freeformType; }
          ]
        else
          c;
      common = {
        _file = toString (m._file or file);
        key = toString (m.key or key);
        disabledModules = m.disabledModules or [ ];
        pos = firstPos m;
      };
    in
    if !isAttrs m then
      throw "optgraph: module ${file} (${key}) does not look like a module"
    else if m ? config || m ? options then
      common
      // {
        imports = m.imports or [ ];
        config = addFreeform (addMeta (m.config or { }));
      }
    else
      common
      // {
        imports = m.require or [ ] ++ m.imports or [ ];
        config = addFreeform (
          removeAttrs m [
            "_class"
            "_file"
            "key"
            "disabledModules"
            "require"
            "imports"
            "freeformType"
          ]
        );
      };

  load =
    file: key: m:
    if lib.isFunction m then
      unify file key (applyArgs m)
    else if isAttrs m then
      if (m._type or "module") == "module" then
        unify file key m
      else if m._type == "if" || m._type == "override" then
        # the synthetic { config = m; } has no meaningful position
        load file key { config = m; } // { pos = null; }
      else
        throw "optgraph: ${file}: a value of type ${m._type} is not a module"
    else if isList m then
      throw "optgraph: ${file}: module imports can't be nested lists"
    else
      let
        v = import m;
      in
      unify (toString m) (toString m) (if lib.isFunction v then applyArgs v else v);

  # Forces what the tree needs from a loaded module (not its config).
  forceModule =
    m: seq m._file (seq m.key (seq (length m.imports) (seq (length m.disabledModules) m)));

  # Placeholder for modules the caller excluded or whose loading threw.
  stub = file: key: {
    _file = file;
    inherit key;
    disabledModules = [ ];
    imports = [ ];
    config = { };
    pos = null;
  };

  # Key a module gets without loading it: paths are keyed by themselves.
  cheapKey = slot: x: if isPath x || isString x then toString x else slot;

  # A node per tree position (not deduplicated). `slot` identifies the position
  # (the fallback key the module system would use) and is what markers carry.
  tree =
    infoOf: parentFile: parentKey: mods:
    lib.imap1 (
      n: x:
      let
        slot = "${noCtx parentKey}:anon-${toString n}";
        excluded = elem slot excludeModules;
        loaded = mark "module" { inherit slot; } (tryEval (forceModule (load parentFile slot x)));
        status =
          if excluded then
            "excluded"
          else if loaded.success then
            "ok"
          else
            "error";
        module = if status == "ok" then loaded.value else stub parentFile (cheapKey slot x);
        info = infoOf n;
      in
      {
        inherit
          slot
          status
          module
          info
          ;
        key = noCtx module.key;
        file = noCtx module._file;
        children = tree (_: info // { depth = info.depth + 1; }) module._file module.key module.imports;
      }
    ) mods;

  # ------------------------------------------------------------ alignment
  # eval-config.nix: getSubModules = baseModules ++ extraModules
  #   ++ [ pkgsModule modulesModule ] ++ map setDefaultModuleLocation modules,
  # and nixpkgs.lib.nixosSystem appends its `nixpkgs.flake.source` module to
  # `modules` (nixpkgs@c59305b:flake.nix:74-91, nixos/lib/eval-config.nix:73-106).
  top = sys.type.getSubModules;

  counts =
    let
      r = tryEval (
        let
          a = sys._module.args;
          # attribute-missing is not catchable: check first
          c =
            if !(a ? baseModules && a ? extraModules && a ? modules) then
              throw "no module lists"
            else
              {
                base = length a.baseModules;
                extra = length a.extraModules;
                user = length a.modules;
              };
        in
        seq c.base (seq c.extra (seq c.user c))
      );
    in
    if r.success then r.value else null;

  userStart = counts.base + counts.extra + 2;

  lengthOk = counts != null && length top == userStart + counts.user;

  userRaw = if lengthOk then lib.sublist userStart counts.user top else [ ];

  # lib.setDefaultModuleLocation file m = { _file = file; imports = [ m ]; }
  isWrapper =
    x:
    isAttrs x
    && !(lib.isFunction x)
    &&
      attrNames x == [
        "_file"
        "imports"
      ]
    && isList x.imports
    && length x.imports == 1;

  wrapped = userRaw != [ ] && all isWrapper userRaw;
  unwrapped = !any isWrapper userRaw;

  wrapperFiles = lib.unique (map (x: toString x._file) userRaw);
  wrapperFile = if wrapped && length wrapperFiles == 1 then head wrapperFiles else null;

  # Top-level node of user module i, below its location wrapper if any.
  userNode =
    i:
    let
      n = elemAt roots (userStart + i);
    in
    if wrapped then head n.children else n;

  # The module nixosSystem injects sets exactly `nixpkgs.flake.source`.
  isInjected =
    node:
    let
      c = node.module.config;
      r = tryEval (
        node.status == "ok"
        && node.module.imports == [ ]
        && isAttrs c
        && !(c ? _type)
        && attrNames c == [ "nixpkgs" ]
        && isAttrs c.nixpkgs
        && !(c.nixpkgs ? _type)
        && attrNames c.nixpkgs == [ "flake" ]
        && isAttrs c.nixpkgs.flake
        && attrNames c.nixpkgs.flake == [ "source" ]
      );
    in
    r.success && r.value;

  injected =
    if lengthOk then filter (i: isInjected (userNode i)) (lib.range 0 (counts.user - 1)) else [ ];

  alignmentProblem =
    if counts == null then
      "_module.args has no baseModules/extraModules/modules (not built by nixos/lib/eval-config.nix?)"
    else if !lengthOk then
      "type.getSubModules has ${toString (length top)} modules, expected ${toString userStart} + ${toString counts.user} from _module.args"
    else if !(wrapped || unwrapped) then
      "only some user modules carry a setDefaultModuleLocation wrapper"
    else if wrapped && wrapperFile == null then
      "user module wrappers disagree on their location: ${toString wrapperFiles}"
    else if injected == [ ] then
      "the module lib.nixosSystem injects (nixpkgs.flake.source) was not found"
    else if
      injected != [
        (counts.user - 1)
      ]
    then
      "the injected nixpkgs.flake.source module is at user index ${toString injected}, expected ${toString (counts.user - 1)}"
    else
      null;

  aligned = alignmentProblem == null;

  # group: "base" | "extra" | "internal" | "user" | "unknown" (not aligned)
  topInfo =
    n:
    let
      i = n - 1;
    in
    {
      depth = 0;
    }
    // (
      if !aligned then
        {
          group = "unknown";
          userIndex = null;
        }
      else if i < counts.base then
        {
          group = "base";
          userIndex = null;
        }
      else if i < counts.base + counts.extra then
        {
          group = "extra";
          userIndex = null;
        }
      else if i < userStart then
        {
          group = "internal";
          userIndex = null;
        }
      else
        {
          group = "user";
          userIndex = i - userStart;
        }
    );

  roots = tree topInfo "<unknown-file>" "" top;

  # ------------------------------------------------------- disabledModules
  # Order-preserving dedup by `key` (lib.unique is quadratic).
  uniqueByKey =
    xs:
    genericClosure {
      startSet = xs;
      operator = _: [ ];
    };
  uniqueStrings = xs: map (x: x.key) (uniqueByKey (map (k: { key = k; }) xs));

  # Every tree position, depth first (duplicates kept).
  flatten = concatMap (n: [ n ] ++ flatten n.children);
  allNodes = flatten roots;

  moduleKey =
    file: m:
    if isString m then
      (if lib.substring 0 1 m == "/" then m else toString modulesPath + "/" + m)
    else if lib.strings.isConvertibleWithToString m then
      toString m
    else if m ? key then
      m.key
    else
      throw "optgraph: ${file}: unsupported disabledModules item";

  disabledOf =
    node:
    let
      r = tryEval (
        let
          ks = map (m: noCtx (moduleKey node.file m)) node.module.disabledModules;
        in
        builtins.deepSeq ks ks
      );
    in
    if r.success then r.value else [ ];

  disabledKeys = uniqueStrings (concatMap disabledOf allNodes);
  disabledSet = listToAttrs (
    map (k: {
      name = k;
      value = true;
    }) disabledKeys
  );
  isDisabled = node: disabledSet ? ${node.key};
  keep = filter (n: !isDisabled n);

  # Loaded modules, unique by key, in the order the module system finds them
  # (same genericClosure as filterModules, lib/modules.nix:566-586).
  live = genericClosure {
    startSet = keep roots;
    operator = n: keep n.children;
  };

  disabledNodes = uniqueByKey (filter isDisabled (roots ++ concatMap (n: n.children) live));
in
{
  inherit
    aligned
    alignmentProblem
    counts
    disabledNodes
    injected
    live
    roots
    wrapped
    wrapperFile
    ;
  injectedIndex = if aligned then counts.user - 1 else null;
  # Nodes whose loading threw (catchable) or was excluded by the caller.
  failedNodes = filter (n: n.status != "ok") allNodes;
  # Keys of the module system's own graph, walked like `live` (skipping
  # disabled subtrees), for the graph-mismatch self-check.
  graphKeys =
    let
      walk = concatMap (g: if g.disabled then [ ] else [ g.key ] ++ walk g.imports);
    in
    uniqueStrings (walk sys.graph);
}
