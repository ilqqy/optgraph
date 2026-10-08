{
  # Plain definition beating the shared input's mkDefault "UTC".
  time.timeZone = "Europe/Berlin";

  # Demonstrates a plain definition beating the nixpkgs default "en_US.UTF-8".
  i18n.defaultLocale = "en_GB.UTF-8";
  i18n.extraLocaleSettings.LC_TIME = "en_GB.UTF-8";

  console.keyMap = "us";
}
