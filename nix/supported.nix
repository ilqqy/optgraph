# nixpkgs releases the extractor is tested against (major.minor of lib.version).
# `minimum`: `.graph` on evalModules results first appears in 25.11.
# `testedUpTo`: newest release covered by CI; newer ones get a warning.
{
  minimum = "25.11";
  testedUpTo = "26.11";
}
