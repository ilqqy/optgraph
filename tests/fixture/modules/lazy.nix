{
  # Nested attrsets are previewed as names only: `boom` is never forced, so
  # this needs no crash recovery.
  fixture.lazy.nested = {
    boom = abort "fixture: a lazy field was forced";
    fine = 1;
  };

  # nixpkgs forces assertion messages only when the assertion fails; the
  # `assertions` option is never previewed.
  assertions = [
    {
      assertion = true;
      message = abort "fixture: an assertion message was forced";
    }
  ];
}
