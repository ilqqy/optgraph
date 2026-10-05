{ lib, ... }:
{
  # mkForce around the whole config: pushed down onto the leaf (priority 50).
  config = lib.mkForce { fixture.layered = "forced block"; };
}
