{ config, pkgs, lib, ... }:

{
  # System configuration - WICHTIG: Muss mit der ursprünglichen Installation übereinstimmen!
  system.stateVersion = "25.11";

  # Boot configuration
  boot = {
    loader.grub = {
      enable = true;
      device = "/dev/sda";
      useOSProber = true;
    };
    
    # Hardware-spezifische Einstellungen (von hardware-configuration.nix)
    initrd.availableKernelModules = [ "ata_piix" "uhci_hcd" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];
    initrd.kernelModules = [ ];
    kernelModules = [ ];
    extraModulePackages = [ ];
  };

  # File Systems (von hardware-configuration.nix)
  fileSystems."/" = {
    device = "/dev/disk/by-uuid/e4b27226-e75f-4cef-9dec-fc0c6f2185ac";
    fsType = "ext4";
  };

  swapDevices = [ ];

  # Platform
  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";

  # Networking
  networking = {
    hostName = "nixos"; # Match existing hostname
    
    # NetworkManager aktivieren (wie auf dem Zielsystem)
    networkmanager.enable = true;
    
    firewall = {
      enable = true;
      allowedTCPPorts = [
        22    # SSH
        80    # HTTP
        443   # HTTPS
        8080  # Docker nginx test
        8000  # CSF-Core Backend
      ];
    };
  };

  # Time zone
  time.timeZone = "Europe/Berlin";

  # Locale settings
  i18n.defaultLocale = "de_DE.UTF-8";
  i18n.extraLocaleSettings = {
    LC_ADDRESS = "de_DE.UTF-8";
    LC_IDENTIFICATION = "de_DE.UTF-8";
    LC_MEASUREMENT = "de_DE.UTF-8";
    LC_MONETARY = "de_DE.UTF-8";
    LC_NAME = "de_DE.UTF-8";
    LC_NUMERIC = "de_DE.UTF-8";
    LC_PAPER = "de_DE.UTF-8";
    LC_TELEPHONE = "de_DE.UTF-8";
    LC_TIME = "de_DE.UTF-8";
  };

  # Console keymap
  console.keyMap = "de";

  # X11 keymap
  services.xserver.xkb = {
    layout = "de";
    variant = "";
  };

  # SSH Server für Remote-Zugriff
  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = "yes"; # Match existing config
      PasswordAuthentication = true; # Match existing config
    };
  };

  # Bestehenden User rootcsf übernehmen
  users.users.rootcsf = {
    isNormalUser = true;
    description = "rootcsf";
    extraGroups = [ "networkmanager" "wheel" "docker" ];
    packages = with pkgs; [];
  };

  # Sudo ohne Passwort für wheel-Gruppe (für automatisiertes Deployment)
  security.sudo.wheelNeedsPassword = false;

  # GnuPG Agent (wie auf dem Zielsystem)
  programs.mtr.enable = true;
  programs.gnupg.agent = {
    enable = true;
    enableSSHSupport = true;
  };

  # Docker aktivieren
  virtualisation.docker = {
    enable = true;
    enableOnBoot = true;
  };

  # System packages
  environment.systemPackages = with pkgs; [
    # Docker tools
    docker-compose

    # System utilities
    curl
    wget
    vim
    htop
    git
    tmux
    
    # Debugging tools
    lsof
    netcat
    tcpdump
  ];

  # Docker Compose service for CSF-Core Backend
  systemd.services.docker-compose-csf-backend = {
    description = "Docker Compose CSF-Core Backend Service";
    after = [ "docker.service" "network-online.target" ];
    requires = [ "docker.service" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      WorkingDirectory = "/etc/csf-core";
      
      # Start containers
      ExecStart = "${pkgs.docker-compose}/bin/docker-compose up -d --remove-orphans";
      
      # Stop containers gracefully
      ExecStop = "${pkgs.docker-compose}/bin/docker-compose down";
      
      # Timeout settings
      TimeoutStartSec = "300";
      TimeoutStopSec = "120";
    };
  };

  # Activation script to setup Docker Compose for CSF-Core Backend
  system.activationScripts.docker-setup = {
    text = ''
      # Create csf-core directory
      mkdir -p /etc/csf-core

      # Create docker-compose.yml for CSF-Core Backend
      cat > /etc/csf-core/docker-compose.yml <<'EOF'
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    container_name: csf-postgres
    environment:
      POSTGRES_USER: csf
      POSTGRES_PASSWORD: csfpassword
      POSTGRES_DB: csf_core
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U csf -d csf_core"]
      interval: 10s
      timeout: 5s
      retries: 5

  backend:
    image: ghcr.io/cs-foundry/csf-core-backend:latest
    container_name: csf-backend
    ports:
      - "8000:8000"
    environment:
      - RUST_LOG=debug
      - DATABASE_URL=postgres://csf:csfpassword@postgres:5432/csf_core
      - JWT_SECRET=supersecretkey_change_me_in_production
      - FRONTEND_URL=http://localhost:3000
    depends_on:
      postgres:
        condition: service_healthy
    restart: unless-stopped

volumes:
  postgres_data:
EOF

      # Create test script
      cat > /root/test-csf-backend.sh <<'EOF'
#!/bin/bash
echo "=== CSF-Core Backend Test ==="
echo "Hostname: $(hostname)"
echo "Date: $(date)"
echo ""
echo "Docker version:"
docker --version
echo ""
echo "Docker Compose version:"
docker-compose --version
echo ""
echo "Docker images:"
docker images
echo ""
echo "Running containers:"
docker ps
echo ""
echo "Docker Compose status:"
cd /etc/csf-core && docker-compose ps
echo ""
echo "=== Network Test ==="
echo "Testing backend API:"
curl -s http://localhost:8000/health || echo "Backend not responding"
echo ""
echo "Testing database connection:"
docker exec csf-postgres pg_isready -U csf -d csf_core || echo "Database not ready"
echo ""
echo "=== Test Complete ==="
EOF
      chmod +x /root/test-csf-backend.sh
    '';
    deps = [];
  };

  # Automatic updates (optional, aber empfohlen)
  system.autoUpgrade = {
    enable = false; # Auf true setzen für automatische Updates
    dates = "04:00";
    allowReboot = false;
  };

  # Nix settings
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
}
