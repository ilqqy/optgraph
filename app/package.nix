# The native app (app/), wrapped with the libraries GPUI loads at run time
# (Vulkan loader, Wayland, X11, xkbcommon) and with the optgraph CLI, which
# `optgraph-app <flakeref>#nixosConfigurations.<host>` runs.
#
# Two derivations: the cargo build (passthru.unwrapped), whose source is app/
# and the two fonts build.rs reads, and a wrapper around it. The CLI changes
# with cli/, nix/ and viewer/; keeping it out of the cargo build means those
# changes rebuild the wrapper (a second), not GPUI (minutes), and a binary
# cache keeps serving the unwrapped build.
{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  runCommand,
  callPackage,
  # The repository; only app/ and the viewer's fonts are used.
  src,
  optgraph,
  version,
}:
let
  runtimeLibs = callPackage ./runtime-libs.nix { };

  unwrapped = rustPlatform.buildRustPackage {
    pname = "optgraph-app-unwrapped";
    inherit version;
    src = lib.fileset.toSource {
      root = src;
      fileset = lib.fileset.unions [
        (src + "/app/Cargo.toml")
        (src + "/app/Cargo.lock")
        (src + "/app/crates")
        # Geist and Geist Mono, read by crates/optgraph-app/build.rs.
        (src + "/viewer/vendor/fonts/Geist.woff2")
        (src + "/viewer/vendor/fonts/GeistMono.woff2")
      ];
    };
    cargoRoot = "app";
    buildAndTestSubdir = "app";
    cargoHash = "sha256-IPx1RYq0TzYqjqUfhSi74cx5XPGv5CqsjHstkK0imtE=";
    cargoBuildFlags = [
      "-p"
      "optgraph-app"
    ];
    # optgraph-core's tests are checks.optgraph-core.
    doCheck = false;

    nativeBuildInputs = [ pkg-config ];
    buildInputs = runtimeLibs;

    meta = {
      description = "optgraph-app without its run-time wrapper (libraries, CLI)";
      mainProgram = "optgraph-app";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
    };
  };
in
runCommand "optgraph-app-${version}"
  {
    nativeBuildInputs = [ makeWrapper ];
    passthru = { inherit unwrapped; };
    meta = unwrapped.meta // {
      description = "Native viewer for optgraph's graph.json: who set an option, at what priority, and why the others lost";
    };
  }
  ''
    makeWrapper ${unwrapped}/bin/optgraph-app $out/bin/optgraph-app \
      --inherit-argv0 \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs} \
      --set-default OPTGRAPH_CLI ${optgraph}/bin/optgraph
  ''
