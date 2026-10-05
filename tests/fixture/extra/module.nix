{ lib, ... }:
{
  fixture.fromExtra = "from the extra input";
  networking.hostName = lib.mkDefault "extra-default";
}
