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
  # A second instance name.
  fixture.home.bob.editor = "emacs";
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

  # A single submodule and a nullOr submodule.
  fixture.single.shell = "dash";
  fixture.maybe.shell = "ksh";

  # Instance-level mkBefore reorders the definitions: degraded to a leaf.
  fixture.ordered.frank = lib.mkBefore { shell = "csh"; };

  # A winning definition that is not an attribute set (like
  # `users.users = mkForce <non-attrset>`): degraded to a leaf.
  fixture.forced = lib.mkForce [ { shell = "not an attrset"; } ];
}
