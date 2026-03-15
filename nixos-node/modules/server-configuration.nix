{ config, pkgs, lib, csf, ... }:

let
  composeDir = "/etc/csf-core";
  csfUpdaterBin = csf.updaterPackage;
in
{
  system.stateVersion = "25.11";

  boot = {
    loader.grub = {
      enable = true;
      device = "/dev/sda";
      useOSProber = true;
    };
    initrd.availableKernelModules = [ "ata_piix" "uhci_hcd" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];
    initrd.kernelModules = [];
    kernelModules = [];
    extraModulePackages = [];
  };

  fileSystems."/" = {
    device = "/dev/disk/by-uuid/e4b27226-e75f-4cef-9dec-fc0c6f2185ac";
    fsType = "ext4";
  };

  swapDevices = [];

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";

  networking = {
    hostName = "csf-node";
    networkmanager.enable = true;
    firewall = {
      enable = true;
      allowedTCPPorts = [ 22 8000 ];
    };
  };

  time.timeZone = "UTC";

  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = "prohibit-password";
      PasswordAuthentication = false;
    };
  };

  users.users.rootcsf = {
    isNormalUser = true;
    description = "rootcsf";
    extraGroups = [ "networkmanager" "wheel" "docker" ];
  };

  security.sudo.wheelNeedsPassword = false;

  virtualisation.docker = {
    enable = true;
    enableOnBoot = true;
  };

  services.csf-daemon = {
    enable = true;
    package = csf.agentPackage;
    apiGateway = "http://localhost:8000";
    heartbeatInterval = 60;
    logLevel = "info";
  };

  environment.systemPackages = with pkgs; [
    docker-compose
    curl
    wget
    vim
    htop
    git
    tmux
    lsof
  ];

  users.users.csf-updater = {
    isSystemUser = true;
    group = "csf-updater";
    extraGroups = [ "docker" ];
    shell = pkgs.shadow;
  };
  users.groups.csf-updater = {};

  systemd.services.csf-updater = {
    description = "CSF Control Plane Updater";
    after = [ "docker.service" "network-online.target" "csf-control-plane.service" ];
    requires = [ "docker.service" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "simple";
      User = "csf-updater";
      Group = "csf-updater";
      EnvironmentFile = "/etc/csf-core/updater.env";
      ExecStart = "${csfUpdaterBin}/bin/csf-updater";
      Restart = "always";
      RestartSec = "10";
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ composeDir ];
    };

    environment = {
      ETCD_ENDPOINTS = "http://localhost:2379";
      ETCD_USERNAME = "csf";
      COMPOSE_FILE = "${composeDir}/docker-compose.yml";
      GHCR_ORG = "csfx-cloud";
      POLL_INTERVAL_SECS = "30";
      RUST_LOG = "info";
      PATH = lib.mkForce "/run/wrappers/bin:/nix/var/nix/profiles/default/bin:/run/current-system/sw/bin";
    };
  };

  systemd.services.csf-control-plane = {
    description = "CSF Control Plane (Docker Compose)";
    after = [ "docker.service" "network-online.target" ];
    requires = [ "docker.service" ];
    wants = [ "network-online.target" ];
    partOf = [ "docker.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      WorkingDirectory = composeDir;
      ExecStartPre = "${pkgs.docker}/bin/docker compose pull --quiet";
      ExecStart = "${pkgs.docker}/bin/docker compose up -d --remove-orphans";
      ExecStop = "${pkgs.docker}/bin/docker compose down";
      TimeoutStartSec = "600";
      TimeoutStopSec = "120";
    };
  };

  system.activationScripts.csf-core-setup = {
    text = ''
      mkdir -p ${composeDir}

      cat > ${composeDir}/docker-compose.yml <<'COMPOSE'
services:
  etcd:
    image: gcr.io/etcd-development/etcd:v3.5.21
    container_name: csf-etcd
    command:
      - etcd
      - --advertise-client-urls=http://etcd:2379
      - --listen-client-urls=http://0.0.0.0:2379
      - --data-dir=/etcd-data
    volumes:
      - etcd_data:/etcd-data
    ports:
      - "2379:2379"
    networks:
      - csf-internal
    restart: unless-stopped

  patroni:
    image: ghcr.io/zalando/spilo-15:3.0-p1
    container_name: csf-patroni
    hostname: patroni
    environment:
      PATRONI_NAME: patroni
      PATRONI_SCOPE: postgres-csf
      PATRONI_ETCD3_HOSTS: "etcd:2379"
      PATRONI_ETCD3_PROTOCOL: http
      PATRONI_POSTGRESQL_DATA_DIR: /home/postgres/pgdata
      PATRONI_POSTGRESQL_LISTEN: "0.0.0.0:5432"
      PATRONI_POSTGRESQL_CONNECT_ADDRESS: "patroni:5432"
      PATRONI_REPLICATION_USERNAME: replicator
      PATRONI_REPLICATION_PASSWORD: replpass
      PATRONI_SUPERUSER_USERNAME: postgres
      PATRONI_SUPERUSER_PASSWORD: postgrespass
      PATRONI_RESTAPI_LISTEN: "0.0.0.0:8008"
      PATRONI_RESTAPI_CONNECT_ADDRESS: "patroni:8008"
      SPILO_CONFIGURATION: |
        bootstrap:
          initdb:
            - auth-host: md5
            - auth-local: trust
          post_bootstrap: /etc/csf-bootstrap.sh
    volumes:
      - patroni_data:/home/postgres/pgdata
      - /etc/csf-core/patroni-bootstrap.sh:/etc/csf-bootstrap.sh:ro
    networks:
      - csf-internal
    depends_on:
      - etcd
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:8008/health | grep -q running || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 60s
    restart: unless-stopped

  api-gateway:
    image: ghcr.io/csfx-cloud/csf-ce-api-gateway:0.2.2-alpha.410
    container_name: csf-api-gateway
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      RUST_LOG: info
      JWT_SECRET: change_me_in_production
      RSA_KEY_SIZE: "4096"
      REGISTRY_SERVICE_URL: http://registry:8001
      SCHEDULER_SERVICE_URL: http://scheduler:8002
      VOLUME_MANAGER_URL: http://volume-manager:8003
      FAILOVER_CONTROLLER_URL: http://failover-controller:8004
      SDN_CONTROLLER_URL: http://sdn-controller:8005
    ports:
      - "8000:8000"
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/api/system/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s

  registry:
    image: ghcr.io/csfx-cloud/csf-ce-registry:0.2.2-alpha.410
    container_name: csf-registry
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      REGISTRY_PORT: "8001"
      RUST_LOG: info
      SCHEDULER_SERVICE_URL: http://scheduler:8002
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped

  scheduler:
    image: ghcr.io/csfx-cloud/csf-ce-scheduler:0.2.2-alpha.410
    container_name: csf-scheduler
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      SCHEDULER_PORT: "8002"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped

  volume-manager:
    image: ghcr.io/csfx-cloud/csf-ce-volume-manager:0.2.2-alpha.410
    container_name: csf-volume-manager
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      VOLUME_MANAGER_PORT: "8003"
      RUST_LOG: info
    volumes:
      - /mnt/csf-volumes:/mnt/csf-volumes
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped

  failover-controller:
    image: ghcr.io/csfx-cloud/csf-ce-failover-controller:0.2.2-alpha.410
    container_name: csf-failover-controller
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      FAILOVER_CONTROLLER_PORT: "8004"
      SCHEDULER_SERVICE_URL: http://scheduler:8002
      VOLUME_MANAGER_URL: http://volume-manager:8003
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped

  sdn-controller:
    image: ghcr.io/csfx-cloud/csf-ce-sdn-controller:0.2.2-alpha.410
    container_name: csf-sdn-controller
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_URL: http://etcd:2379
      SDN_CONTROLLER_PORT: "8005"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    networks:
      - csf-internal
    restart: unless-stopped

volumes:
  etcd_data:
  patroni_data:

networks:
  csf-internal:
    driver: bridge
COMPOSE

      cat > ${composeDir}/patroni-bootstrap.sh <<'BOOTSTRAP'
#!/bin/bash
psql -U postgres -c "CREATE USER csf WITH PASSWORD 'csfpassword';"
psql -U postgres -c "CREATE DATABASE csf_core OWNER csf;"
psql -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE csf_core TO csf;"
BOOTSTRAP
      chmod +x ${composeDir}/patroni-bootstrap.sh
    '';
    deps = [];
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
}
