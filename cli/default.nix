{
  lib,
  writeShellApplication,
  coreutils,
  gawk,
  gnugrep,
  gnused,
  jq,
  # optgraph's own source and its NAR hash: the Nix lib is loaded from it with
  # builtins.fetchTree inside a pure `nix eval --apply` (docs/module-system-notes.md topic 8).
  src,
  narHash,
  toolVersion,
}:
writeShellApplication {
  name = "optgraph";
  # `nix` itself is deliberately not pinned: the user's nix and its settings are used.
  runtimeInputs = [
    coreutils
    gawk
    gnugrep
    gnused
    jq
  ];
  text = ''
    OPTGRAPH_SRC=${lib.escapeShellArg "${src}"}
    OPTGRAPH_NARHASH=${lib.escapeShellArg narHash}
    OPTGRAPH_VERSION=${lib.escapeShellArg toolVersion}
  ''
  + builtins.readFile ./optgraph.sh;
}
