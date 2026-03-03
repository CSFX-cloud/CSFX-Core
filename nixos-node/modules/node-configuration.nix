{ config, pkgs, lib, ... }:

{
  system.stateVersion = "24.11";

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
