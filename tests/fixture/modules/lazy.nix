{ lib, ... }:
{
  # Nested attrsets are previewed as names only: `boom` is never forced, so
  # this needs no crash recovery.
  fixture.lazy.nested = {
    boom = abort "fixture: a lazy field was forced";
    fine = 1;
  };

  # A fixpoint package set (like boot.kernelPackages) is previewed as names
  # only, even at the top level: `removed` stands for an alias of a removed
  # package, which throws when forced.
  fixture.packageSet = lib.makeExtensible (self: {
    fine = 1;
    removed = throw "fixture: a removed alias was forced";
  });

  # nixpkgs forces assertion messages only when the assertion fails; the
  # `assertions` option is never previewed.
  assertions = [
    {
      assertion = true;
      message = abort "fixture: an assertion message was forced";
    }
  ];
}
