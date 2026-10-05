# optgraph extraction library.
#
# Pure Nix without a nixpkgs dependency of its own: every `lib` function used
# comes from the evaluated configuration's nixpkgs, so the module-system
# semantics that are reconstructed match the ones that produced the config.
{
  toolVersion ? "dev",
}:
{
  extract = import ./extract.nix { inherit toolVersion; };
  supportedNixpkgs = import ./supported.nix;
}
