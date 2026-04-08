{ config, pkgs, lib, csf, versions, ... }:

let
  updateUnitsModule = import ../../../CSFX-Infra/modules/update-units.nix;
  composeDir = "/etc/csf-core";
in
{
  imports = [ updateUnitsModule ];

  system.stateVersion = "25.05";

  boot = {
    loader.grub = {
      enable = true;
      device = "/dev/sda";
    };
    initrd.availableKernelModules = [ "ata_piix" "uhci_hcd" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];
  };

  fileSystems."/" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "ext4";
  };

  fileSystems."/boot" = {
    device = "/dev/disk/by-label/boot";
    fsType = "vfat";
  };

  swapDevices = [];

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";

  networking = {
    hostName = "csf-node";
    useDHCP = true;
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

  users.users.admin = {
    isNormalUser = true;
    extraGroups = [ "wheel" "docker" ];
    openssh.authorizedKeys.keys = [];
  };

  security.sudo.wheelNeedsPassword = false;

  virtualisation.docker = {
    enable = true;
    enableOnBoot = true;
  };

  users.users.csf-agent = {
    isSystemUser = true;
    group = "csf-agent";
    home = "/var/lib/csf-daemon";
    createHome = true;
  };
  users.groups.csf-agent = {};
  users.groups.csf-updater = {};

  systemd.tmpfiles.rules = [
    "d /var/lib/csf-daemon 0750 csf-agent csf-agent -"
    "d /var/lib/csf 0750 csf-agent csf-updater -"
    "f /var/lib/csf/update_trigger 0660 csf-agent csf-updater -"
    "d /var/lib/csf-updater 0750 root root -"
    "d /var/lib/csf-updater/infra.git 0750 root root -"
  ];

  systemd.services.csf-agent = {
    description = "CSF Agent Daemon";
    wantedBy = [ "multi-user.target" ];
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    serviceConfig = {
      ExecStart = "${csf.agentPackage}/bin/csf-agent";
      User = "csf-agent";
      Group = "csf-agent";
      Restart = "on-failure";
      RestartSec = "10s";
      PrivateTmp = true;
      ProtectSystem = "strict";
      ReadWritePaths = [ "/var/lib/csf-daemon" "/var/lib/csf" ];
      NoNewPrivileges = true;
    };
    environment = {
      CSF_GATEWAY_URL = "http://localhost:8000";
      CSF_HEARTBEAT_INTERVAL = "60";
      RUST_LOG = "info";
    };
  };

  systemd.services.csf-updater = {
    description = "CSF GitOps Updater";
    wantedBy = [ "multi-user.target" ];
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    serviceConfig = {
      ExecStart = "${csf.updaterPackage}/bin/csf-updater";
      Restart = "on-failure";
      RestartSec = "10s";
      StateDirectory = "csf-updater";
    };
    environment = {
      ETCD_ENDPOINTS = "http://localhost:2379";
      INFRA_REPO_GITHUB = "csfx-cloud/CSFX-Infra";
      INFRA_REPO_BRANCH = "main";
      INFRA_REPO_MIRROR_URL = "https://github.com/csfx-cloud/CSFX-Infra.git";
      INFRA_REPO_MIRROR_DIR = "/var/lib/csf-updater/infra.git";
      POLL_INTERVAL_SECS = "120";
      RUST_LOG = "info";
    };
  };

  services.csf-update-units = {
    enable = true;
    nixCacheUrl = "http://localhost:5000";
    nixCachePublicKey = "";
  };

  nix.settings = {
    experimental-features = [ "nix-command" "flakes" ];
    trusted-users = [ "root" ];
  };

  system.activationScripts.csf-core-compose = {
    text = ''
      mkdir -p ${composeDir}

      cat > ${composeDir}/docker-compose.yml <<'COMPOSE'
services:
  etcd:
    image: gcr.io/etcd-development/etcd:v3.5.21
    container_name: csf-etcd
    command:
      - etcd
      - --advertise-client-urls=http://0.0.0.0:2379
      - --listen-client-urls=http://0.0.0.0:2379
      - --data-dir=/etcd-data
    volumes:
      - etcd_data:/etcd-data
    ports:
      - "2379:2379"
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
    image: ghcr.io/csfx-cloud/csf-ce-api-gateway@${versions.csf.images.api-gateway.digest}
    container_name: csf-api-gateway
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      JWT_SECRET: change_me_in_production
      ETCD_ENDPOINTS: http://etcd:2379
      REGISTRY_SERVICE_URL: http://registry:8001
      SCHEDULER_SERVICE_URL: http://scheduler:8002
      VOLUME_MANAGER_URL: http://volume-manager:8003
      FAILOVER_CONTROLLER_URL: http://failover-controller:8004
      SDN_CONTROLLER_URL: http://sdn-controller:8005
      RUST_LOG: info
    ports:
      - "8000:8000"
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

  registry:
    image: ghcr.io/csfx-cloud/csf-ce-registry@${versions.csf.images.registry.digest}
    container_name: csf-registry
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      REGISTRY_PORT: "8001"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

  scheduler:
    image: ghcr.io/csfx-cloud/csf-ce-scheduler@${versions.csf.images.scheduler.digest}
    container_name: csf-scheduler
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      SCHEDULER_PORT: "8002"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

  volume-manager:
    image: ghcr.io/csfx-cloud/csf-ce-volume-manager@${versions.csf.images.volume-manager.digest}
    container_name: csf-volume-manager
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      VOLUME_MANAGER_PORT: "8003"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

  failover-controller:
    image: ghcr.io/csfx-cloud/csf-ce-failover-controller@${versions.csf.images.failover-controller.digest}
    container_name: csf-failover-controller
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      FAILOVER_CONTROLLER_PORT: "8004"
      SCHEDULER_SERVICE_URL: http://scheduler:8002
      VOLUME_MANAGER_URL: http://volume-manager:8003
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

  sdn-controller:
    image: ghcr.io/csfx-cloud/csf-ce-sdn-controller@${versions.csf.images.sdn-controller.digest}
    container_name: csf-sdn-controller
    environment:
      DATABASE_URL: postgres://csf:csfpassword@patroni:5432/csf_core
      ETCD_ENDPOINTS: http://etcd:2379
      SDN_CONTROLLER_PORT: "8005"
      RUST_LOG: info
    depends_on:
      patroni:
        condition: service_healthy
    restart: unless-stopped

volumes:
  etcd_data:
  patroni_data:
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

  systemd.services.csf-control-plane = {
    description = "CSF Control Plane (Docker Compose)";
    after = [ "docker.service" "network-online.target" ];
    requires = [ "docker.service" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      WorkingDirectory = composeDir;
      ExecStart = "${pkgs.docker}/bin/docker compose up -d --remove-orphans";
      ExecStop = "${pkgs.docker}/bin/docker compose down";
      TimeoutStartSec = "600";
    };
  };

  environment.systemPackages = with pkgs; [
    docker-compose
    curl
    git
    jq
    etcd
  ];
}
