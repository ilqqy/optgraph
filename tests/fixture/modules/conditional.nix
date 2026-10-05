{ lib, ... }:
{
  config = lib.mkMerge [
    # Leaf mkIf false: kept as an inactive definition.
    { fixture.conditional = lib.mkIf false 42; }

    # mkIf true: active, no condition reported.
    { fixture.conditionalTrue = lib.mkIf true "yes"; }

    # A condition that throws: condition "mkIf-error", winner unknown.
    { fixture.conditionalError = lib.mkIf (throw "fixture: this condition throws") "never"; }

    # mkIf false around a whole block: the walk pushes the condition down; the
    # override inside is never forced, so its priority is unknown (null).
    (lib.mkIf false { fixture.prio = lib.mkOverride 10 "never"; })
  ];
}
