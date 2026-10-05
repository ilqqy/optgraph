# extract :: args -> JSON-ready attrset (see schema/graph.schema.json)
#
# Stages per option, each forced inside its own error-context marker so the CLI
# can localize crashes builtins.tryEval cannot catch:
#   reconstruct -> selfcheck -> preview
# Callers degrade an option by listing it in `noPreview` / `noSelfcheck`, or drop
# it with `exclude`; modules are dropped by tree slot with `excludeModules`.
{ toolVersion }:
{
  # A nixosSystem / evalModules result (`nixosConfigurations.<host>`).
  config,
  # The configuration's own nixpkgs lib (nixosSystem results carry it).
  lib ? config.lib or config.pkgs.lib or null,
  # Flake input name -> source root path (e.g. from `nix flake archive --json`).
  inputs ? { },
  # Source root of the flake defining the configuration (origin "user").
  selfRoot ? null,
  # Disable the scope filter: every declared option, every module.
  all ? false,
  # Restrict the options to these locs (lists of strings); null = no restriction.
  only ? null,
  exclude ? [ ],
  noPreview ? [ ],
  noSelfcheck ? [ ],
  excludeModules ? [ ],
  preview ? true,
  selfcheck ? true,
  # List every definition; by default nixpkgs definitions that neither win
  # nor come from a non-nixpkgs module are only counted (`omitted`).
  includeAllDefinitions ? false,
  # "full" | "scope" (only list the in-scope option locs)
  mode ? "full",
  host ? null,
  generatedAt ? null,
}:
let
  inherit (builtins)
    any
    attrNames
    attrValues
    compareVersions
    concatMap
    concatStringsSep
    deepSeq
    elemAt
    filter
    foldl'
    genericClosure
    groupBy
    head
    isString
    length
    listToAttrs
    match
    seq
    sort
    stringLength
    toJSON
    tryEval
    typeOf
    ;

  supported = import ./supported.nix;
  P = import ./preview.nix;
  R = import ./reconstruct.nix { inherit lib; };

  mark = stage: subject: builtins.addErrorContext "optgraph-marker:${stage}:${toJSON subject}";

  strict = x: deepSeq x x;

  # Attribute names must not carry string context (store paths from inputs do).
  noCtx = builtins.unsafeDiscardStringContext;

  setOf =
    xs:
    listToAttrs (
      map (x: {
        name = noCtx (toJSON x);
        value = true;
      }) xs
    );

  warn = code: subject: message: { inherit code subject message; };

  # ------------------------------------------------------------ version gate
  version = lib.version or "unknown";
  majorMinor = lib.versions.majorMinor version;

  fatal =
    if !(config ? options && config ? type && config ? _module) then
      {
        code = "not-a-configuration";
        message = "the value is not a module system evaluation (no options/type/_module)";
      }
    else if lib == null then
      {
        code = "not-a-configuration";
        message = "the configuration has neither .lib nor .pkgs.lib (not built by nixpkgs.lib.nixosSystem?); pass `lib` explicitly";
      }
    else if compareVersions majorMinor supported.minimum < 0 then
      {
        code = "nixpkgs-unsupported";
        message = "nixpkgs ${version} is older than the minimum supported ${supported.minimum}";
      }
    else if !(config ? graph) then
      {
        code = "nixpkgs-unsupported";
        message = "the configuration has no .graph (lib.evalModules of nixpkgs ${version} predates it; need ${supported.minimum} or newer)";
      }
    else if !basics.success then
      {
        code = "config-broken";
        message = "evaluating _module.specialArgs / _module.args of the configuration threw (throw/assert); does the configuration evaluate at all?";
      }
    else
      null;

  # Everything below reads these; a configuration that cannot compute them
  # (e.g. no nixpkgs.hostPlatform) is reported as fatal instead of crashing.
  basics = tryEval (
    deepSeq [
      (attrNames config._module.specialArgs)
      (attrNames config._module.args)
    ] true
  );

  # ------------------------------------------------------------ collection
  col = import ./collect.nix { inherit lib mark; } {
    sys = config;
    inherit excludeModules;
  };

  modulesPath = config._module.specialArgs.modulesPath or null;
  nixpkgsRoot = if modulesPath == null then null else noCtx (dirOf (dirOf (toString modulesPath)));

  # ---------------------------------------------------------------- origins
  # `<root>/./sub` (relative path inputs) -> `<root>/sub`, no trailing slash.
  normRoot = r: noCtx (lib.removeSuffix "/" (lib.replaceStrings [ "/./" ] [ "/" ] (toString r)));
  selfRoot' = if selfRoot == null then null else normRoot selfRoot;

  inputGroups = groupBy (n: normRoot inputs.${n}) (sort lib.lessThan (attrNames inputs));
  reserved = filter (r: r != null) [
    nixpkgsRoot
    selfRoot'
  ];
  inputRoots = lib.mapAttrsToList (root: names: {
    inherit root names;
    origin = "input:${head names}";
  }) (removeAttrs inputGroups reserved);

  rootList = sort (a: b: stringLength a.root > stringLength b.root) (
    lib.optional (nixpkgsRoot != null) {
      root = nixpkgsRoot;
      origin = "nixpkgs";
    }
    ++ lib.optional (selfRoot' != null) {
      root = selfRoot';
      origin = "user";
    }
    ++ inputRoots
  );

  originOfFile =
    f:
    let
      f' = normRoot f;
      m = lib.findFirst (r: f' == r.root || lib.hasPrefix (r.root + "/") f') null rootList;
    in
    if !isString f then
      "unknown"
    else if m == null then
      "unknown"
    else
      m.origin;

  isPathModule = n: n.key == n.file && lib.hasPrefix "/" n.file;
  posOf = n: n.module.pos or null;

  # Is an anonymous module's `_file` only a location fallback rather than
  # where its code is? (eval-config gives inline user modules nixpkgs'
  # flake.nix; top-level anonymous modules get "<unknown-file>".)
  fileIsFallback =
    n:
    let
      p = posOf n;
    in
    n.file == "<unknown-file>"
    || (col.aligned && n.file == col.wrapperFile)
    || (
      p != null
      && p.file != n.file
      && originOfFile n.file == "nixpkgs"
      && originOfFile p.file != "nixpkgs"
    );

  # One rule for anonymous modules, aligned or not: a real `_file` wins (the
  # module lives in that file); otherwise the position of its code decides:
  # user code -> "user-inline", inputs and nixpkgs -> their origin.
  anonOrigin =
    n:
    let
      p = posOf n;
      o = if p == null then null else originOfFile p.file;
    in
    if !(fileIsFallback n) then
      originOfFile n.file
    else if o == "user" then
      "user-inline"
    else if o != null then
      o
    else if n.info.group == "user" || n.info.group == "extra" then
      "user-inline"
    else
      "unknown";

  nodeOrigin =
    n:
    let
      g = n.info.group;
    in
    if isPathModule n then
      originOfFile n.file
    else if g == "base" || g == "internal" then
      "nixpkgs"
    else if g == "user" && n.info.userIndex == col.injectedIndex then
      "nixpkgs" # injected by lib.nixosSystem
    else
      anonOrigin n;

  isWrapperNode = n: col.aligned && col.wrapped && n.info.group == "user" && n.info.depth == 0;

  liveNodes = filter (n: !isWrapperNode n) col.live;
  originByKey = listToAttrs (
    map (n: {
      name = noCtx n.key;
      value = nodeOrigin n;
    }) liveNodes
  );
  nonNixpkgsNodes = filter (n: originByKey.${noCtx n.key} != "nixpkgs") liveNodes;
  fileByKey = listToAttrs (
    map (n: {
      name = noCtx n.key;
      value = n.file;
    }) liveNodes
  );

  # ------------------------------------------------------- definitions walk
  walkNode =
    trie: n:
    mark "walk" { inherit (n) slot; } (
      tryEval (
        let
          ds = R.walk {
            inherit (config) options;
            inherit (n) file;
            module = n.key;
            inherit trie;
          } n.module.config;
        in
        seq (length ds) ds
      )
    );

  walkAll =
    trie: nodes:
    let
      rs = map (n: {
        inherit n;
        r = walkNode trie n;
      }) nodes;
    in
    {
      defs = concatMap (x: if x.r.success then x.r.value else [ ]) rs;
      failed = map (x: x.n) (filter (x: !x.r.success) rs);
    };

  uniqueLocs =
    locs:
    attrValues (
      listToAttrs (
        map (l: {
          name = toJSON l;
          value = l;
        }) locs
      )
    );

  # Pass A: option paths defined by non-nixpkgs modules (structure only).
  passA = walkAll null nonNixpkgsNodes;

  declaredLocs = map (o: o.loc) (
    filter (o: head o.loc != "_module") (lib.collect lib.isOption config.options)
  );

  scopeLocs = if all then declaredLocs else uniqueLocs (map (d: d.loc) passA.defs);

  onlySet = setOf only;
  locs = if only == null then scopeLocs else filter (l: onlySet ? ${toJSON l}) scopeLocs;

  # Pass B: every module, pruned to the selected options.
  passB = walkAll (if all && only == null then null else R.trieOf locs) liveNodes;
  byLoc = groupBy (d: toJSON d.loc) passB.defs;

  # ------------------------------------------------------------ per option
  excludeSet = setOf exclude;
  noPreviewSet = setOf noPreview;
  noSelfcheckSet = setOf noSelfcheck;
  # Never previewed: their attrsets carry fields (assertion messages) that
  # nixpkgs forces only when an assertion fails; forcing them can throw.
  neverPreviewSet = setOf [
    [ "assertions" ]
    [ "warnings" ]
  ];
  optionDefaultPriority = (lib.mkOptionDefault null).priority;

  # Sorted, deduplicated list of strings.
  sortUnique =
    xs:
    attrNames (
      listToAttrs (
        map (x: {
          name = noCtx x;
          value = null;
        }) xs
      )
    );

  optionOf =
    loc:
    let
      key = toJSON loc;
      path = lib.showOption loc;
      subject = { inherit loc path; };
      opt = lib.getAttrFromPath loc config.options;
      raws = byLoc.${key} or [ ];

      classified = concatMap (r: map (c: c // { inherit (r) module; }) (R.classify r.file r.raw)) raws;

      recon = mark "reconstruct" subject (
        strict (
          let
            entries = map (d: {
              kind = "definition";
              file = toString d.file;
              inherit (d) module priority;
              active = builtins.all (c: c == true) d.conditions;
              condition =
                if any (c: c == false) d.conditions then
                  "mkIf-false"
                else if any (c: c == null) d.conditions then
                  "mkIf-error"
                else
                  null;
            }) classified;
            declarations =
              let
                r = tryEval (strict (map toString opt.declarations));
              in
              if r.success then r.value else [ ];
            defaultEntry = {
              kind = "default";
              file = if declarations == [ ] then null else head declarations;
              module = null;
              priority = optionDefaultPriority;
              active = true;
              condition = null;
            };
            definitions = entries ++ lib.optional (opt ? default) defaultEntry;
            # A definition whose value or condition could not be read makes the
            # winner unknown (the module system itself would throw here).
            uncertain = any (d: (d.active && d.priority == null) || d.condition == "mkIf-error") definitions;
            prios = map (d: d.priority) (filter (d: d.active && d.priority != null) definitions);
            highestPrio = if uncertain || prios == [ ] then null else foldl' lib.min (head prios) prios;
            type =
              let
                r = tryEval opt.type.description;
              in
              if r.success && isString r.value then r.value else null;
          in
          {
            inherit
              definitions
              highestPrio
              type
              uncertain
              ;
            declaredIn = declarations;
            winners = filter (
              i:
              let
                d = elemAt definitions i;
              in
              d.active && d.priority != null && d.priority == highestPrio
            ) (lib.range 0 (length definitions - 1));
            errors =
              concatMap (x: x) (
                lib.imap0 (
                  i: d:
                  lib.optional d.probeFailed "definition ${toString i}: throw/assert while reading its value"
                  ++ lib.optional (
                    lib.elem null d.conditions && !(lib.elem false d.conditions)
                  ) "definition ${toString i}: an mkIf condition threw or is not a bool"
                ) classified
              )
              ++ lib.optional uncertain "winner unknown: a definition could not be classified";
          }
        )
      );

      selfcheckResult =
        if !selfcheck || noSelfcheckSet ? ${key} then
          null
        else
          seq recon (
            mark "selfcheck" subject (
              strict (
                let
                  hp = tryEval opt.highestPrio;
                  fs = tryEval (strict (map toString opt.files));
                  winnerFiles = sortUnique (map (i: (elemAt recon.definitions i).file) recon.winners);
                  refPrio = if hp.value >= 9999 then null else hp.value;
                  refFiles = sortUnique fs.value;
                in
                if !hp.success || !fs.success then
                  {
                    status = "skipped";
                    message = "the module system's highestPrio/files threw (throw/assert)";
                  }
                else if recon.uncertain then
                  {
                    status = "skipped";
                    message = "a definition could not be classified";
                  }
                else if refPrio == recon.highestPrio && winnerFiles == refFiles then
                  { status = "ok"; }
                else
                  {
                    status = "mismatch";
                    message = "reconstructed highestPrio ${toJSON recon.highestPrio} files ${toJSON winnerFiles}, module system ${toJSON refPrio} files ${toJSON refFiles}";
                  }
              )
            )
          );

      contents = map (d: d.content) classified;

      # ---- output stage, after the self-check: which definitions to list
      winnerSet = listToAttrs (
        map (i: {
          name = toString i;
          value = true;
        }) recon.winners
      );
      originOfDef =
        d:
        if d.module != null then
          originByKey.${noCtx d.module} or (originOfFile d.file)
        else
          originOfFile d.file;
      indices = lib.range 0 (length recon.definitions - 1);
      keep =
        if includeAllDefinitions then
          indices
        else
          # A single nixpkgs winner is kept; with several winners (mergeable
          # types: lists, attrsets) nixpkgs ones are only counted.
          filter (
            i:
            (singleWinner && winnerSet ? ${toString i}) || originOfDef (elemAt recon.definitions i) != "nixpkgs"
          ) indices;
      singleWinner = length recon.winners == 1;
      keepSet = listToAttrs (
        map (i: {
          name = toString i;
          value = true;
        }) keep
      );
      dropped = filter (i: !(keepSet ? ${toString i})) indices;
      omitted = {
        nixpkgsActive = length (filter (i: (elemAt recon.definitions i).active) dropped);
        nixpkgsInactive = length (filter (i: !(elemAt recon.definitions i).active) dropped);
      };

      # Option paths that look like secrets: non-scalar values are redacted.
      sensitive =
        match ".*(password|passwd|secret|token|private|credential|api[_-]?key).*" (lib.toLower path)
        != null;
      isScalar =
        v:
        let
          t = tryEval (typeOf v);
        in
        t.success
        && lib.elem t.value [
          "bool"
          "null"
          "int"
          "float"
        ];
      redacted = {
        text = "<redacted>";
        failed = false;
      };
      previewValue = v: f: if sensitive && !(isScalar v) then redacted else f v;

      previews =
        if !preview || noPreviewSet ? ${key} || neverPreviewSet ? ${key} then
          map (_: null) keep
        else
          seq recon (
            mark "preview" subject (
              strict (
                map (
                  i:
                  let
                    d = elemAt recon.definitions i;
                  in
                  if d.kind == "default" then
                    previewValue opt.default (_: P.previewDefault opt)
                  else if d.active && !(elemAt classified i).probeFailed then
                    previewValue (elemAt contents i) P.preview
                  else
                    null
                ) keep
              )
            )
          );

      previewErrors = concatMap (x: x) (
        lib.zipListsWith (
          i: p: lib.optional (p != null && p.failed) "definition ${toString i}: preview hit throw/assert"
        ) keep previews
      );

      errors = recon.errors ++ previewErrors;

      # `file` is dropped when it is the file of the definition's module.
      outDef =
        i: p:
        let
          d = elemAt recon.definitions i;
          sameFile = d.module != null && (fileByKey.${noCtx d.module} or null) == d.file;
        in
        (if sameFile then removeAttrs d [ "file" ] else d)
        // {
          valuePreview = if p == null then null else p.text;
        };

      keptDefs = map (i: elemAt recon.definitions i) keep;

      warnings =
        lib.optional (noPreviewSet ? ${key}) (
          warn "preview-crash" path "uncatchable crash while previewing; previews disabled for this option"
        )
        ++ lib.optional (noSelfcheckSet ? ${key}) (
          warn "selfcheck-crash" path
            "uncatchable crash in the self-check; self-check skipped for this option"
        )
        ++ lib.optional (selfcheckResult != null && selfcheckResult.status == "mismatch") (
          warn "reconstruction-mismatch" path selfcheckResult.message
        )
        ++ lib.optional (selfcheckResult != null && selfcheckResult.status == "skipped") (
          warn "selfcheck-skipped" path selfcheckResult.message
        );
    in
    if excludeSet ? ${key} then
      {
        record = {
          inherit path loc;
          declaredIn = [ ];
          type = null;
          definitions = [ ];
          winners = [ ];
          highestPrio = null;
          omitted = {
            nixpkgsActive = 0;
            nixpkgsInactive = 0;
          };
          error = "excluded after an uncatchable crash during reconstruct (see meta.warnings)";
        };
        warnings = [
          (warn "eval-crash" path "uncatchable crash while reconstructing definitions; option excluded")
        ];
        moduleIds = [ ];
        files = [ ];
      }
    else
      seq recon (
        seq selfcheckResult (
          seq previews {
            record = {
              inherit path loc omitted;
              inherit (recon) declaredIn type highestPrio;
              # winners re-indexed into the listed definitions
              winners = filter (j: winnerSet ? ${toString (elemAt keep j)}) (lib.range 0 (length keep - 1));
              definitions = lib.zipListsWith outDef keep previews;
              error = if errors == [ ] then null else concatStringsSep "; " errors;
            };
            inherit warnings;
            moduleIds = filter (m: m != null) (map (d: d.module) keptDefs);
            files = filter (f: f != null) (map (d: d.file) keptDefs);
          }
        )
      );

  # passB is forced first, inside its "walk" markers, so a crash in a module
  # walk is never blamed on the first option.
  optionResults = seq (length passB.defs) (
    map (x: optionOf x.loc) (
      sort (a: b: a.path < b.path) (
        map (l: {
          loc = l;
          path = lib.showOption l;
        }) (if all then locs else filter (l: byLoc ? ${toJSON l}) locs)
      )
    )
  );

  # ---------------------------------------------------------------- modules
  childKeys = n: map (c: c.key) n.children;

  parentsOf = groupBy (x: noCtx x.child) (
    concatMap (
      n:
      map (c: {
        child = c;
        parent = n.key;
      }) (childKeys n)
    ) liveNodes
  );

  includedKeys =
    if all then
      map (n: n.key) liveNodes
    else
      let
        seeds = map (n: n.key) nonNixpkgsNodes;
        ancestors = map (x: x.key) (genericClosure {
          startSet = map (k: { key = k; }) seeds;
          operator = x: map (p: { key = p.parent; }) (parentsOf.${noCtx x.key} or [ ]);
        });
        # Direct imports of user modules (e.g. nixpkgs profiles), so their
        # `imports` edges resolve.
        imported = concatMap childKeys nonNixpkgsNodes;
      in
      ancestors ++ imported ++ definers;

  # Modules of the listed definitions, so every definition's `module` resolves.
  definers = concatMap (o: o.moduleIds) optionResults;

  disabledKeys = map (n: n.key) col.disabledNodes;
  included = setOf (includedKeys ++ disabledKeys);

  moduleRecord =
    disabled: n:
    let
      origin = if disabled then nodeOrigin n else originByKey.${noCtx n.key};
      p = posOf n;
    in
    {
      id = n.key;
      inherit (n) file;
      inherit origin disabled;
      imports =
        if disabled then [ ] else sortUnique (filter (k: included ? ${noCtx (toJSON k)}) (childKeys n));
      # Only the module that is itself an entry of nixosSystem's `modules`.
      modulesIndex =
        if origin == "user-inline" && n.info.group == "user" && n.info.depth == userDepth then
          n.info.userIndex
        else
          null;
      position = if isPathModule n || p == null then null else "${toString p.file}:${toString p.line}";
    };

  userDepth = if col.wrapped then 1 else 0;

  modules =
    map (moduleRecord false) (filter (n: included ? ${noCtx (toJSON n.key)}) liveNodes)
    ++ map (moduleRecord true) col.disabledNodes;

  # --------------------------------------------------------------- warnings
  graphCheck =
    let
      ours = sortUnique (map (n: n.key) col.live);
      theirs = sortUnique col.graphKeys;
      oursSet = setOf ours;
      theirsSet = setOf theirs;
      onlyOurs = filter (k: !(theirsSet ? ${noCtx (toJSON k)})) ours;
      onlyTheirs = filter (k: !(oursSet ? ${noCtx (toJSON k)})) theirs;
    in
    if col.failedNodes != [ ] || ours == theirs then
      [ ]
    else
      [
        (warn "graph-mismatch" null
          "re-collected ${toString (length ours)} modules, the module system's graph has ${toString (length theirs)}; only re-collected: ${toJSON (lib.take 3 onlyOurs)}; only in graph: ${toJSON (lib.take 3 onlyTheirs)}"
        )
      ];

  # Files the output actually attributes: listed modules (file and position)
  # and the listed definitions' own files.
  usedFiles =
    map (m: m.file) modules
    ++ map (m: m.position) (filter (m: m.position != null) modules)
    ++ concatMap (o: o.files) optionResults;
  rootUsed = root: any (f: f == root || lib.hasPrefix (root + "/") (normRoot f)) usedFiles;

  globalWarnings =
    lib.optional (!col.aligned) (
      warn "alignment-failed" null
        "${col.alignmentProblem}; module origins fall back to files and positions, inline modules may be misattributed"
    )
    ++ lib.optional (compareVersions majorMinor supported.testedUpTo > 0) (
      warn "nixpkgs-untested" null
        "nixpkgs ${version} is newer than the newest tested release ${supported.testedUpTo}"
    )
    ++ map (
      r:
      warn "input-ambiguous" r.root "inputs ${toJSON r.names} share one source; attributed to ${r.origin}"
    ) (filter (r: r ? names && length r.names > 1 && rootUsed r.root) rootList)
    ++ map (
      n:
      if n.status == "excluded" then
        warn "eval-crash" n.slot "uncatchable crash while loading or walking this module; module excluded"
      else
        warn "module-error" n.slot "loading the module threw (throw/assert); it is treated as empty"
    ) col.failedNodes
    ++ map (
      slot:
      warn "module-error" slot
        "walking the module's config threw (throw/assert); its definitions are missing"
    ) (sortUnique (map (n: n.slot) (lib.optionals (!all) passA.failed ++ passB.failed)))
    ++ graphCheck;

  nixpkgsRev =
    let
      r = tryEval config.config.system.nixos.revision;
    in
    if r.success && isString r.value then r.value else null;

  result = {
    meta = {
      inherit host generatedAt toolVersion;
      inherit nixpkgsRev;
      nixpkgsVersion = version;
      scope = if all then "all" else "user";
      attribution = if col.aligned then "aligned" else "file-based";
      complete = true;
      warnings = globalWarnings ++ concatMap (o: o.warnings) optionResults;
    };
    inherit modules;
    options = map (o: o.record) optionResults;
  };
in
if fatal != null then
  { inherit fatal; }
else if mode == "scope" then
  {
    scope = map (l: {
      loc = l;
      path = lib.showOption l;
    }) scopeLocs;
  }
else
  result
