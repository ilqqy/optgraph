{ lib, ... }:
{
  users.users.alice = {
    description = lib.mkForce "Alice (forced)";
    # A false mkIf inside the instance: listed on users.users.alice.home.
    home = lib.mkIf false "/nowhere";
  };
}
