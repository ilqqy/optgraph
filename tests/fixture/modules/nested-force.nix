{ lib, ... }:
{
  fixture.home.alice.theme = lib.mkForce "forced";
  fixture.accounts.dave.shell = lib.mkForce "tcsh";
}
