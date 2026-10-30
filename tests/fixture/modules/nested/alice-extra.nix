# Imported inside the fixture.home.alice instance.
{ lib, ... }:
{
  theme = lib.mkDefault "dark";
  files.notes.text = "from a file imported inside the instance";
}
