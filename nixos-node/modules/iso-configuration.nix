{ config, pkgs, lib, csf, versions, ... }:

let
  updateUnitsModule = import ../../../CSFX-Infra/modules/update-units.nix;

  installScript = pkgs.writeShellScript "csf-install" ''
    set -euo pipefail

    DISK=""

    for dev in sda vda nvme0n1; do
      if [ -b "/dev/$dev" ]; then
        DISK="/dev/$dev"
        break
      fi
    done

    if [ -z "$DISK" ]; then
      echo "[csf-install] ERROR: no suitable disk found" >&2
      exit 1
    fi

    echo "[csf-install] target disk: $DISK"

    if [[ "$DISK" == *nvme* ]]; then
      PART_BOOT="${DISK}p1"
      PART_ROOT="${DISK}p2"
    else
      PART_BOOT="${DISK}1"
      PART_ROOT="${DISK}2"
    fi

    parted "$DISK" -- mklabel gpt
    parted "$DISK" -- mkpart ESP fat32 1MB 512MB
    parted "$DISK" -- mkpart primary ext4 512MB 100%
    parted "$DISK" -- set 1 esp on

    mkfs.fat -F 32 -n boot "$PART_BOOT"
    mkfs.ext4 -L nixos "$PART_ROOT"

    mount "$PART_ROOT" /mnt
    mkdir -p /mnt/boot
    mount "$PART_BOOT" /mnt/boot

    echo "[csf-install] partitioning complete, running nixos-install"

    nixos-install \
      --no-root-passwd \
      --flake /iso/csf-flake#csf-server

    echo "[csf-install] installation complete — rebooting in 5s"
    sleep 5
    reboot
  '';

  logoText = builtins.readFile ../logo.txt;

  motd = pkgs.writeText "csf-motd" ''
    ${logoText}

    ╔══════════════════════════════════════════════════════════════════╗
    ║                    CSF Node Installer                            ║
    ║                                                                  ║
    ║  Automatische Installation startet in 10 Sekunden.              ║
    ║  CTRL+C zum Abbrechen und manuellem Eingriff.                    ║
    ║                                                                  ║
    ║  Nach der Installation:                                          ║
    ║    - csf-agent verbindet sich mit dem API Gateway                ║
    ║    - Updates laufen automatisch via GitOps                       ║
    ║                                                                  ║
    ╚══════════════════════════════════════════════════════════════════╝
  '';
in
{
  imports = [
    <nixpkgs/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix>
    updateUnitsModule
  ];

  system.stateVersion = "25.05";

  isoImage.volumeID = "CSF-NODE";
  isoImage.edition = lib.mkForce "csf";
  isoImage.prependToMenuLabel = "CSF Node Installer — ";
  isoImage.makeEfiBootable = true;
  isoImage.makeUsbBootable = true;

  isoImage.storeContents = [
    csf.agentPackage
    csf.updaterPackage
  ];

  isoImage.contents = [
    {
      source = ../../../CSFX-Infra;
      target = "/csf-flake";
    }
  ];

  boot.kernelParams = [
    "console=ttyS0,115200n8"
    "console=tty0"
    "quiet"
  ];

  boot.loader.timeout = lib.mkForce 10;

  networking = {
    hostName = "csf-installer";
    useDHCP = true;
    firewall.enable = false;
  };

  time.timeZone = "UTC";

  services.getty.autologinUser = lib.mkForce "root";

  users.users.root = {
    initialPassword = "";
    shell = pkgs.bash;
  };

  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = "yes";
      PasswordAuthentication = true;
    };
  };

  environment.etc."motd".source = motd;

  systemd.services.csf-autoinstall = {
    description = "CSF automatic node installer";
    after = [ "network-online.target" "getty.target" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      Type = "oneshot";
      ExecStartPre = "${pkgs.coreutils}/bin/sleep 10";
      ExecStart = installScript;
      StandardOutput = "journal+console";
      StandardError = "journal+console";
    };
  };

  nix.settings = {
    experimental-features = [ "nix-command" "flakes" ];
    trusted-users = [ "root" ];
  };

  environment.systemPackages = with pkgs; [
    git
    curl
    parted
    dosfstools
    e2fsprogs
    jq
    vim
  ];
}
