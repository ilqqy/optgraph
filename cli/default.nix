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
  # The built viewer (packages.viewer); `--html` embeds the graph into it.
  viewer,
}:
let
  embedHtml = writeShellApplication {
    name = "optgraph-embed-html";
    runtimeInputs = [
      coreutils
      gnugrep
      gnused
      jq
    ];
    text = builtins.readFile ./embed-html.sh;
  };
in
writeShellApplication {
  name = "optgraph";
  # `nix` itself is deliberately not pinned: the user's nix and its settings are used.
  runtimeInputs = [
    coreutils
    embedHtml
    gawk
    gnugrep
    gnused
    jq
  ];
  text = ''
    OPTGRAPH_SRC=${lib.escapeShellArg "${src}"}
    OPTGRAPH_NARHASH=${lib.escapeShellArg narHash}
    OPTGRAPH_VERSION=${lib.escapeShellArg toolVersion}
    OPTGRAPH_VIEWER=${lib.escapeShellArg "${viewer}/index.html"}
  ''
  + builtins.readFile ./optgraph.sh;
  passthru = { inherit embedHtml; };
}
