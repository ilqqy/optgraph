# The native app (app/), wrapped with the libraries GPUI loads at run time
# (Vulkan loader, Wayland, X11, xkbcommon) and with the optgraph CLI, which
# `optgraph-app <flakeref>#nixosConfigurations.<host>` runs.
{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  callPackage,
  # The repository; only app/ and the viewer's fonts are used.
  src,
  optgraph,
  version,
}:
let
  runtimeLibs = callPackage ./runtime-libs.nix { };
in
rustPlatform.buildRustPackage {
  pname = "optgraph-app";
  inherit version;
  src = lib.fileset.toSource {
    root = src;
    fileset = lib.fileset.unions [
      (src + "/app/Cargo.toml")
      (src + "/app/Cargo.lock")
      (src + "/app/crates")
      (src + "/viewer/vendor/fonts") # Geist, read by crates/optgraph-app/build.rs
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

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];
  buildInputs = runtimeLibs;

  postFixup = ''
    wrapProgram $out/bin/optgraph-app \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs} \
      --set-default OPTGRAPH_CLI ${optgraph}/bin/optgraph
  '';

  meta = {
    description = "Native viewer for optgraph's graph.json: who set an option, at what priority, and why the others lost";
    mainProgram = "optgraph-app";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
  };
}
