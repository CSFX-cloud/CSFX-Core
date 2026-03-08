{ config, pkgs, lib, csf, ... }:

{
  system.stateVersion = "25.05";

  boot.loader.grub = {
    enable = true;
    device = "/dev/sda";
  };

  networking = {
    hostName = "csf-node";
    firewall = {
      enable = true;
      allowedTCPPorts = [];
    };
  };

  time.timeZone = "UTC";

  users.users.root.hashedPassword = "!";

  services.openssh.enable = false;

  services.csf-daemon = {
    enable = true;
    package = csf.agentPackage;
    apiGateway = "http://gateway.csf.local:8000";
    registrationToken = "csf-bootstrap.change_me";
    heartbeatInterval = 60;
    logLevel = "info";
  };

  nix = {
    settings = {
      experimental-features = [ "nix-command" "flakes" ];
      auto-optimise-store = true;
    };
    gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 30d";
    };
  };

  environment.systemPackages = with pkgs; [
    curl
  ];
}
