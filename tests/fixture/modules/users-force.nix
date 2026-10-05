{ lib, ... }: { users.users.alice.description = lib.mkForce "Alice (forced)"; }
