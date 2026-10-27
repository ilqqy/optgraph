{ lib, ... }:
{
  # A false mkIf around the whole instance definition: the module system never
  # looks inside; listed (inactive) on users.users.alice.extraGroups.
  users.users.alice = lib.mkIf false { extraGroups = [ "wheel" ]; };
  fixture.home.alice.theme = "plain";
  fixture.home.carol = {
    theme = "lost at the instance level";
    editor = "ed";
  };
  fixture.accounts.dave.shell = "zsh";
  fixture.accounts.erin.greeting = "hi erin";
  fixture.single = {
    greeting = "from nested-plain.nix";
  };
  fixture.ordered.frank.greeting = "first";
}
