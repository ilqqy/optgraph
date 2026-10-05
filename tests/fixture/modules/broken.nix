{ lib, ... }:
{
  # throw: caught by builtins.tryEval, reported in the option's `error`.
  fixture.throws = throw "fixture: this definition throws";

  # abort: not catchable; the CLI must exclude the option and retry.
  fixture.aborting = abort "fixture: this definition aborts";

  # Aborts only when the value is forced to WHNF (the mkForce wrapper is fine):
  # reconstruction survives, the self-check and the preview crash and degrade.
  fixture.whnfAbort = lib.mkForce (abort "fixture: aborts when forced to WHNF");
}
