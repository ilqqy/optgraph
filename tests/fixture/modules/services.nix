{
  systemd.services.fixture-hello = {
    description = "optgraph fixture service";
    script = "echo hello";
  };

  # A key that services.openssh.settings only accepts through its freeformType.
  services.openssh.settings.ClientAliveInterval = 60;
}
