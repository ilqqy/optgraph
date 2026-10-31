# Nested attribution: definitions inside submodule instances.
{ lib, ... }:
{
  # Function-style instance with its own import; the name and the special
  # argument osConfig come from the instance's evaluation.
  fixture.home.alice =
    { name, osConfig, ... }:
    {
      imports = [ ./nested/alice-extra.nix ];
      editor = "vim for ${name} on ${osConfig.networking.hostName}";
      files.todo.text = "nested submodule";
    };
  # A large instance (more than six modules, each file a submodule of its own):
  # the graph starts with it collapsed into one pill.
  fixture.home.zoe =
    { ... }:
    {
      imports = map (n: { files.${n}.text = "file ${n}"; }) [
        "a"
        "b"
        "c"
        "d"
      ];
      # A long, deep path: truncated in the viewers' lists.
      files."config/some-application/with/a/rather/long/path/settings.conf".text =
        lib.mkDefault "from the default";
    };
  # A second instance name. Its mkDefault packages lose to the declaration
  # module's two (listed: none of the winners).
  fixture.home.bob.editor = "emacs";
  fixture.home.bob.packages = lib.mkDefault [ "mine" ];
  # The instance-level mkForce wins; nested-plain.nix's carol definition loses
  # as a whole (shadowedAt).
  fixture.home.carol = lib.mkForce { theme = "forced instance"; };

  # Instances that break: one throws when evaluated, one aborts (the CLI
  # excludes just that instance).
  fixture.home.broken = { ... }: throw "fixture: this instance throws";
  fixture.home.aborts = { ... }: abort "fixture: this instance aborts";

  # mkDefault vs plain vs mkForce inside an instance (with nested-plain.nix
  # and nested-force.nix).
  fixture.accounts.dave.shell = lib.mkDefault "bash";
  # A false mkIf at the sub-option and at the instance level.
  fixture.accounts.dave.greeting = lib.mkIf false "never";
  fixture.accounts.erin = lib.mkIf false { shell = "fish"; };

  # lazyAttrsOf: an instance whose value throws breaks only itself.
  fixture.lazyAccounts.fine.shell = "zsh";
  fixture.lazyAccounts.bad = throw "fixture: this lazy instance throws";

  # mkForce on the whole option: nested-plain.nix's definition never reaches
  # the instance.
  fixture.allForced = lib.mkForce { gina.shell = "forced"; };

  # A single submodule and a nullOr submodule.
  fixture.single.shell = "dash";
  fixture.maybe.shell = "ksh";

  # Instance-level mkBefore reorders the definitions: degraded to a leaf.
  fixture.ordered.frank = lib.mkBefore { shell = "csh"; };

  # A winning definition that is not an attribute set (like
  # `users.users = mkForce <non-attrset>`): degraded to a leaf.
  fixture.forced = lib.mkForce [ { shell = "not an attrset"; } ];
}
