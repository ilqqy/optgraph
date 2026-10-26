# `cargo test` of optgraph-core alone: its own Cargo.lock (the workspace's
# entries it uses, nothing of GPUI), so the check vendors a dozen crates.
{ pkgs }:
pkgs.stdenv.mkDerivation {
  pname = "optgraph-core-tests";
  version = "0.1.0";
  src = ./crates/optgraph-core;
  cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./crates/optgraph-core/Cargo.lock; };
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.rustc
    pkgs.rustPlatform.cargoSetupHook
  ];
  buildPhase = ''
    runHook preBuild
    cargo test --offline --frozen
    runHook postBuild
  '';
  installPhase = "touch $out";
}
