# Nested attribution: definitions inside submodule instances
# (users.users.<name>, systemd.services.<name>, home-manager.users.<name>, ...).
# See docs/module-system-notes.md topic 9.
#
# For a supported option (`submodule`, `attrsOf`/`lazyAttrsOf` of one, `nullOr`
# of one) the module system keeps every instance's own evaluation in
# `valueMeta`. Each instance is re-collected with nix/collect.nix and walked
# like the top level; its definition modules are matched to the definitions
# that produced them, so every nested definition keeps the module that wrote
# it. Whatever cannot be matched exactly makes the instance a leaf again, with
# a `nested-degraded` warning: nothing is attributed by guesswork.
#
# Laziness: like nix/reconstruct.nix. Instance-level values and conditions are
# forced (under tryEval) as far as the module system forces them to list the
# instances; definitions that lost at an enclosing level, or sit under a false
# mkIf there, are walked only when their value is a plain attrset.
{
  lib,
  R,
  collect,
  optionOf,
  originOfFile,
  isPathModule,
  warn,
  mark,
  strict,
  maxDepth,
  excludeNestingSet,
  excludeInstancesSet,
  optionDefaultPriority,
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
    foldl'
    genericClosure
    groupBy
    head
    isAttrs
    length
    listToAttrs
    sort
    tail
    toJSON
    tryEval
    unsafeGetAttrPos
    ;

  noCtx = builtins.unsafeDiscardStringContext;

  # A forced boolean that is false when forcing it threw (throw/assert).
  holds =
    x:
    let
      r = tryEval x;
    in
    r.success && r.value == true;

  setOf =
    xs:
    listToAttrs (
      map (x: {
        name = noCtx x;
        value = true;
      }) xs
    );

  uniqueLocs =
    locs:
    builtins.attrValues (
      listToAttrs (
        map (l: {
          name = toJSON l;
          value = l;
        }) locs
      )
    );

  firstPos =
    m:
    let
      names = attrNames m;
    in
    if names == [ ] then null else unsafeGetAttrPos (head names) m;

  # "single" | "attrs" | null (a leaf)
  shapeOf =
    opt:
    let
      r = tryEval (
        let
          t = opt.type;
          el = t.nestedTypes.elemType or null;
          elName = if el == null then null else el.name or null;
          shape =
            if t.name == "submodule" then
              "single"
            else if (t.name == "attrsOf" || t.name == "lazyAttrsOf") && elName == "submodule" then
              "attrs"
            else if t.name == "nullOr" && elName == "submodule" then
              "single"
            else
              null;
        in
        builtins.seq shape shape
      );
    in
    if r.success then r.value else null;

  isActive = c: all (x: x == true) c.conditions;
  isKnown = c: all (x: x != null) c.conditions;

  # The module system's definition order: modules in reverse order, each
  # module's own definitions in order (lib/modules.nix: reverseList modules).
  msOrder =
    xs:
    concatMap (g: g.items) (
      foldl' (
        acc: x:
        if acc != [ ] && (head acc).module == x.module then
          [ ((head acc) // { items = (head acc).items ++ [ x ]; }) ] ++ tail acc
        else
          [
            {
              inherit (x) module;
              items = [ x ];
            }
          ]
          ++ acc
      ) [ ] xs
    );

  walkPlain =
    args: cfg:
    tryEval (
      let
        ds = R.walk args cfg;
      in
      builtins.seq (length ds) ds
    );

  # Where an anonymous module inside an instance comes from. A real `_file`
  # wins, unless the code itself sits in the user's flake (an inline module
  # handed to the submodule, e.g. through home-manager.sharedModules).
  innerOrigin =
    n:
    let
      p = n.module.pos or null;
      fromPos = if p == null then null else originOfFile p.file;
      fromFile = originOfFile n.file;
    in
    if isPathModule n then
      fromFile
    else if fromPos == "user" && fromFile != "user" then
      "user-inline"
    else if n.file == "<unknown-file>" then
      (if fromPos == null then "unknown" else fromPos)
    else
      fromFile;

  # expandOption :: ctx -> option result -> { scopes, warnings }
  # `scopes` is flat: every instance below this option, recursively.
  expandOption =
    ctx: res:
    let
      nest = res.nest;
      none = {
        scopes = [ ];
        warnings = [ ];
      };
    in
    if nest == null || ctx.depth >= maxDepth then
      none
    else
      let
        shape = shapeOf nest.opt;
        key = toJSON nest.loc;
        subject = {
          inherit (nest) loc path;
        };
        degradeOption = msg: {
          scopes = [ ];
          warnings = [ (warn "nested-degraded" nest.path msg) ];
        };
        userDefs = filter (c: c.module != null && ctx.userSideDef c) nest.classified;

        isSurvivor = c: c.shadowedAt == null && isActive c && c.priority == nest.highestPrio;
        survivors = filter isSurvivor nest.classified;
        lost = filter (c: !(isSurvivor c)) nest.classified;
        # The option default is a definition too (first, lib/modules.nix:1144).
        defaultSurvivor = lib.optional (nest.hasDefault && nest.highestPrio == optionDefaultPriority) {
          file = nest.declFile;
          module = null;
          content = nest.opt.default;
          conditions = [ ];
          priority = optionDefaultPriority;
          probeFailed = false;
          shadowedAt = null;
        };
        expected = defaultSurvivor ++ msOrder survivors;

        hasOrder = any (s: holds ((s.content._type or null) == "order")) expected;
        plainAttrs = v: isAttrs v && !(v ? _type);
        contentsAttrs = all (s: holds (plainAttrs s.content)) expected;

        # Instance names written by a user-side definition, among those that exist.
        enumeration = mark "instances" subject (
          tryEval (
            strict (
              let
                fromUser = concatMap (
                  c:
                  let
                    r = tryEval (strict (if plainAttrs c.content then attrNames c.content else [ ]));
                  in
                  if r.success then r.value else [ ]
                ) (filter isKnown userDefs);
                present = setOf (attrNames nest.opt.valueMeta.attrs);
              in
              filter (n: present ? ${n}) (lib.unique fromUser)
            )
          )
        );

        instances =
          if shape == "attrs" then
            map (name: {
              inherit name;
              loc = nest.loc ++ [ name ];
            }) enumeration.value
          else
            [
              {
                name = null;
                inherit (nest) loc;
              }
            ];

        expanded = map (expandInstance ctx {
          inherit
            nest
            shape
            expected
            lost
            ;
        }) instances;
      in
      if shape == null || userDefs == [ ] then
        none
      else if excludeNestingSet ? ${key} then
        {
          scopes = [ ];
          warnings = [
            (warn "nested-crash" nest.path
              "uncatchable crash while listing the option's instances; they are not expanded"
            )
          ];
        }
      else if nest.uncertain then
        degradeOption "the option's winner is unknown; its instances are not expanded"
      else if nest.highestPrio == null then
        none
      else if hasOrder then
        degradeOption "mkBefore/mkAfter/mkOrder on the option's definitions reorders them; its instances are not expanded"
      else if shape == "attrs" && !contentsAttrs then
        degradeOption "a winning definition is not a plain attribute set; its instances are not expanded"
      else if shape == "attrs" && !enumeration.success then
        degradeOption "listing the option's instances threw (throw/assert); they are not expanded"
      else
        {
          scopes = concatMap (e: e.scopes) expanded;
          warnings = concatMap (e: e.warnings) expanded;
        };

  expandInstance =
    ctx:
    {
      nest,
      shape,
      expected,
      lost,
    }:
    inst:
    let
      instLoc = inst.loc;
      instPath = lib.showOption instLoc;
      ikey = toJSON instLoc;
      subject = {
        loc = instLoc;
        path = instPath;
      };
      idOf = k: "«${instPath}»${k}";
      degrade = msg: {
        scopes = [ ];
        warnings = [ (warn "nested-degraded" instPath msg) ];
      };

      # ---- instance level (attrsOf/lazyAttrsOf): a second round of mkIf/mkOverride
      perName = concatMap (
        s:
        if s.content ? ${inst.name} then
          map (
            c:
            c
            // {
              inherit (s) module;
              shadowedAt = null;
            }
          ) (R.classify s.file s.content.${inst.name})
        else
          [ ]
      ) expected;
      uncertainI = any (c: c.probeFailed || !(isKnown c)) perName;
      priosI = map (c: c.priority) (filter isActive perName);
      hpI = if priosI == [ ] then null else foldl' lib.min (head priosI) priosI;
      isSurvivorI = c: isActive c && c.priority == hpI;
      instSurvivors = if shape == "attrs" then filter isSurvivorI perName else expected;
      instLost = if shape == "attrs" then filter (c: !(isSurvivorI c)) perName else [ ];
      orderI = any (c: holds ((c.content._type or null) == "order")) instSurvivors;

      # ---- the module system's evaluation of the instance
      vm = nest.opt.valueMeta;
      conf = if shape == "attrs" then vm.attrs.${inst.name}.configuration else vm.configuration;
      hasConf = shape == "attrs" || holds (vm ? configuration);
      subs = conf.type.getSubModules;
      nDecl = length nest.opt.type.getSubModules;
      defMods = lib.drop (nDecl + 2) subs;
      fileStr = f: noCtx (toString f);
      aligned =
        length subs == nDecl + 2 + length instSurvivors
        && map (m: fileStr (m._file or "")) defMods == map (s: fileStr s.file) instSurvivors;

      col = collect {
        sys = conf;
        prefix = instLoc;
      };
      roots = col.roots;
      graphOk =
        let
          sortU = xs: attrNames (setOf xs);
        in
        sortU (map (n: n.key) col.live) == sortU col.graphKeys;

      # ---- which inner modules are what
      # roots: [ name-default ] ++ <nDecl declaration wrappers> ++ [ name ] ++ <definitions>
      isShorthand = i: (elemAt subs i) ? config;
      defIndices = lib.range (nDecl + 2) (length roots - 1);
      defNodeOf = i: if isShorthand i then elemAt roots i else head (elemAt roots i).children;
      hiddenKeys = setOf (
        map (n: n.key) ([ (elemAt roots 0) ] ++ lib.sublist 1 nDecl roots ++ [ (elemAt roots (nDecl + 1)) ])
        ++ map (i: (elemAt roots i).key) (filter (i: !(isShorthand i)) defIndices)
      );
      # Everything above indexes `roots`/`subs` by position: only valid once
      # the shapes line up (elemAt/head out of range are not catchable).
      shapesOk =
        holds aligned
        && holds (
          length roots == length subs
          && all (i: isShorthand i || length (elemAt roots i).children == 1) defIndices
        );

      visit =
        parentUser: declSide: n:
        let
          origin = innerOrigin n;
          user =
            if declSide then origin == "user" || origin == "user-inline" else parentUser && origin != "nixpkgs";
        in
        [
          {
            inherit (n) key;
            inherit origin user;
            owner = null;
            position = null;
          }
        ]
        ++ concatMap (visit user declSide) n.children;

      defEntries = concatMap (
        i:
        let
          d = defNodeOf i;
          s = elemAt instSurvivors (i - nDecl - 2);
          origin = if s.module == null then originOfFile s.file else ctx.originOfDef s;
          user = s.module != null && ctx.userSideDef s;
          p = if isShorthand i then firstPos (elemAt subs i).config else d.module.pos or null;
        in
        [
          {
            inherit (d) key;
            inherit origin user;
            owner = s.module;
            position = if p == null || isPathModule d then null else "${toString p.file}:${toString p.line}";
          }
        ]
        ++ concatMap (visit user false) d.children
      ) (if shapesOk then defIndices else [ ]);

      declEntries = concatMap (w: concatMap (visit false true) w.children) (lib.sublist 1 nDecl roots);

      # One record per key (a module imported twice is user-side if either path is).
      info = lib.mapAttrs (
        _: es:
        (head es)
        // {
          user = any (e: e.user) es;
          owner = lib.findFirst (o: o != null) null (map (e: e.owner) es);
          position = lib.findFirst (o: o != null) null (map (e: e.position) es);
        }
      ) (groupBy (e: noCtx e.key) (defEntries ++ declEntries));

      visible = filter (n: !(hiddenKeys ? ${n.key})) col.live;
      userNodes = filter (n: (info.${n.key} or { user = false; }).user) visible;

      walkNode =
        trie: n:
        walkPlain {
          inherit (conf) options;
          inherit (n) file;
          module = idOf n.key;
          inherit trie;
        } n.module.config;

      passA = map (walkNode null) userNodes;

      # ---- definitions that never reached the instance
      # (lost at the option or the instance level, or under a false mkIf there).
      # Only user-side ones: the module system never evaluates these values, and
      # library code under a false mkIf may refer to things that do not exist
      # (an uncatchable `attribute missing`); non-user definitions would only be
      # counted anyway.
      shadowSources =
        map (c: {
          inherit c;
          value = if shape == "attrs" then c.content.${inst.name} else c.content;
          inactive = !(isActive c);
          shadowedAt =
            if c.shadowedAt != null then
              c.shadowedAt
            else if isActive c then
              nest.path
            else
              null;
          hasName = shape != "attrs" || holds (isAttrs c.content && c.content ? ${inst.name});
        }) (filter (c: isKnown c && c.module != null && ctx.userSideDef c) lost)
        ++ map (c: {
          inherit c;
          value = c.content;
          inactive = !(isActive c);
          shadowedAt = if isActive c then instPath else null;
          hasName = true;
        }) (filter (c: isKnown c && c.module != null && ctx.userSideDef c) instLost);

      shadowed = concatMap (
        src:
        let
          v = src.value;
          # module syntax in a non-shorthand definition: only `config` holds definitions
          cfg = if v ? config && !(conf.options ? config) then v.config else v;
          r = walkPlain {
            inherit (conf) options;
            inherit (src.c) file module;
            trie = null;
          } cfg;
        in
        if src.hasName && holds (isAttrs v) && r.success then
          map (
            d:
            d
            // {
              inherit (src) shadowedAt;
              raw = if src.inactive then lib.mkIf false d.raw else d.raw;
              userOwned = ctx.userSideDef src.c;
            }
          ) r.value
        else
          [ ]
      ) shadowSources;

      touched = uniqueLocs (
        concatMap (r: if r.success then map (d: d.loc) r.value else [ ]) passA
        ++ map (d: d.loc) (filter (d: d.userOwned) shadowed)
      );
      touchedSet = setOf (map toJSON touched);
      trie = R.trieOf touched;
      passB = map (walkNode trie) visible;
      walkFailed = any (r: !r.success) (passA ++ passB);
      raws = concatMap (r: if r.success then r.value else [ ]) passB;
      extraRaws = filter (d: touchedSet ? ${toJSON d.loc}) shadowed;
      byLoc = groupBy (d: toJSON d.loc) raws;
      extra = groupBy (d: toJSON d.loc) extraRaws;

      # Each step runs only when the previous ones hold (an attribute that is
      # missing or an index out of range would not be catchable).
      check = mark "instance" subject (
        strict (
          if !hasConf then
            "none"
          else if shape == "attrs" && !(holds (!uncertainI)) then
            "uncertain"
          else if holds orderI then
            "order"
          else if !(holds (attrNames conf.options != null)) then
            "evaluates"
          else if !shapesOk then
            "aligned"
          else if !(holds (col.failedNodes == [ ])) then
            "load"
          else if !(holds graphOk) then
            "graph"
          else if
            !(holds (!walkFailed))
            || !(tryEval (strict [
              (attrNames info)
              (length raws)
              (length extraRaws)
              touched
            ])).success
          then
            "walk"
          else
            "ok"
        )
      );

      # ---- the instance as a scope of its own
      fileById = listToAttrs (
        map (n: {
          name = idOf n.key;
          value = n.file;
        }) visible
      );
      infoById = lib.mapAttrs' (k: v: lib.nameValuePair (idOf k) v) info;
      ctx' = {
        inherit (conf) options;
        prefix = instLoc;
        instance = instPath;
        parent = nest.path;
        depth = ctx.depth + 1;
        inherit byLoc extra;
        originOfDef =
          d:
          if d.module != null && infoById ? ${d.module} then
            infoById.${d.module}.origin
          else
            ctx.originOfDef d;
        userSideDef =
          d:
          if d.module != null && infoById ? ${d.module} then infoById.${d.module}.user else ctx.userSideDef d;
        fileOf = id: fileById.${id} or (ctx.fileOf id);
      };

      relLocs = map (x: x.loc) (
        sort (a: b: a.path < b.path) (
          map (l: {
            loc = l;
            path = lib.showOption l;
          }) (filter (l: byLoc ? ${toJSON l} || extra ? ${toJSON l}) touched)
        )
      );
      results = map (optionOf ctx') relLocs;
      children = map (expandOption ctx') results;

      # Modules: user-side ones, their ancestors and direct imports; the rest
      # only when a listed definition or an owner link needs them (extract.nix).
      parentsOf = groupBy (x: noCtx x.child) (
        concatMap (
          n:
          map (c: {
            child = c.key;
            parent = n.key;
          }) n.children
        ) visible
      );
      ancestors = map (x: x.key) (genericClosure {
        startSet = map (n: { key = n.key; }) userNodes;
        operator =
          x:
          map (p: { key = p.parent; }) (
            filter (p: !(hiddenKeys ? ${p.parent})) (parentsOf.${noCtx x.key} or [ ])
          );
      });
      baseKeys = setOf (ancestors ++ concatMap (n: map (c: c.key) n.children) userNodes);
      modules = map (
        n:
        let
          i =
            info.${n.key} or {
              origin = innerOrigin n;
              user = false;
              owner = null;
              position = null;
            };
          p = n.module.pos or null;
        in
        {
          id = idOf n.key;
          base = baseKeys ? ${n.key};
          inherit (i) owner;
          childIds = map (c: idOf c.key) n.children;
          record = {
            id = idOf n.key;
            inherit (n) file;
            inherit (i) origin;
            disabled = false;
            modulesIndex = null;
            position =
              if i.position != null then
                i.position
              else if isPathModule n || p == null then
                null
              else
                "${toString p.file}:${toString p.line}";
            scope = instPath;
          }
          // lib.optionalAttrs (i.owner != null) { inherit (i) owner; };
        }
      ) visible;
    in
    if excludeInstancesSet ? ${ikey} then
      {
        scopes = [ ];
        warnings = [
          (warn "nested-crash" instPath
            "uncatchable crash while expanding this instance; it stays part of its option"
          )
        ];
      }
    else if check == "none" then
      {
        scopes = [ ];
        warnings = [ ];
      }
    else if check == "uncertain" then
      degrade "an instance-level definition could not be classified (throw/assert or a failed mkIf condition)"
    else if check == "order" then
      degrade "mkBefore/mkAfter/mkOrder on the instance's definitions reorders them; definitions cannot be matched to their modules"
    else if check == "evaluates" then
      degrade "the instance does not evaluate (throw/assert)"
    else if check == "aligned" then
      degrade "the instance's definition modules do not line up with the reconstructed winning definitions"
    else if check == "load" then
      degrade "loading an instance module threw (throw/assert)"
    else if check == "graph" then
      degrade "the re-collected instance modules differ from the instance's own module graph"
    else if check == "walk" then
      degrade "walking an instance module threw (throw/assert)"
    else
      {
        scopes = [
          {
            instance = instPath;
            inherit results modules;
          }
        ]
        ++ concatMap (c: c.scopes) children;
        warnings = concatMap (c: c.warnings) children;
      };
in
{
  inherit expandOption shapeOf;
}
