{ hostName, singleNode }:
{ lib, pkgs, ... }:
let
  binDir = "/opt/csfx-dev/aarch64-unknown-linux-musl/debug";
  rbdSimDir = "/var/lib/csfx-agent/rbd-sim";

  rbdSim = pkgs.writeShellApplication {
    name = "rbd";
    runtimeInputs = [ pkgs.bash pkgs.util-linux pkgs.coreutils pkgs.findutils pkgs.gawk pkgs.jq ];
    text = ''exec bash /csfx-dev/rbd-sim.sh "$@"'';
  };

  devUnits = {
    csfx-migrate = "csfx-migrate";
    csfx-api-gateway = "api-gateway";
    csfx-registry = "registry";
    csfx-scheduler = "scheduler";
    csfx-volume-manager = "volume-manager";
    csfx-failover-controller = "failover-controller";
    csfx-sdn-controller = "sdn-controller";
    csfx-object-storage = "object-storage";
    csfx-site-builder = "site-builder";
    csfx-agent = "csfx-agent";
  };
in
{
  networking = {
    inherit hostName;
    firewall.enable = lib.mkForce false;
  };

  users.mutableUsers = lib.mkForce true;

  systemd.tmpfiles.rules = [ "d ${rbdSimDir} 0750 csfx-agent csfx-agent -" ];

  services = {
    csfx-agent = {
      enable = true;
      gatewayUrl = "https://localhost:8000";
      enableSsh = true;
      enableFirecracker = true;
      advertiseInterface = "enp0s2";
      localDns.servers = [ "127.0.0.1" ];
    };
    csfx-ingress = {
      enable = true;
      dnsListenAddresses = [ "lo" "enp0s2" ];
    };
    csfx-cp = {
      enable = true;
      dbUrl = "";
    };
    csfx-setup = {
      enable = true;
      dataPart = "/dev/vdb";
    };
    csfx-cluster = {
      enable = true;
      vip = lib.mkIf (hostName == "cp1") {
        address = "192.168.105.200";
        interface = "enp0s2";
      };
    };
    csfx-garage = {
      enable = true;
      replicationFactor = lib.mkIf singleNode 1;
    };
    csfx-openbao.enable = true;
    csfx-site-registry.enable = true;
  };

  systemd.services = lib.mkMerge [
    (lib.mapAttrs
      (_: bin: { serviceConfig.ExecStart = lib.mkForce "${binDir}/${bin}"; })
      devUnits)
    {
      csfx-api-gateway.environment.STATIC_DIR = lib.mkForce "/csfx-ui";
      csfx-site-builder.serviceConfig.EnvironmentFile = lib.mkAfter [ "-/etc/csfx/site-dev.env" ];
      csfx-updater.enable = lib.mkForce false;
      csfx-guest-init-link.serviceConfig.ExecStart = lib.mkForce "${pkgs.coreutils}/bin/install -m 0755 ${binDir}/csfx-guest-init /var/lib/csfx-agent/csfx-guest-init";
      csfx-mgmt-up.serviceConfig.ExecStart = lib.mkForce "${binDir}/csfx-agent --mgmt-up";
      csfx-master-join.serviceConfig.ExecStart = lib.mkForce "${binDir}/csfx-agent --join-master";
      csfx-agent.environment.CSFX_SSH_CONTROL = lib.mkForce "0";
      csfx-agent.environment.CSFX_RBD_SIM_DIR = rbdSimDir;
      csfx-agent.path = [ rbdSim ];
      sshd.wantedBy = lib.mkForce [ "multi-user.target" ];
    }
  ];

  security.wrappers.csfx-authorized-keys.source = lib.mkForce "${binDir}/csfx-agent";

  services.openssh = {
    authorizedKeysFiles = lib.mkForce [ ".ssh/authorized_keys" "/etc/ssh/authorized_keys.d/%u" ];
    settings.AllowUsers = lib.mkForce [ "*" ];
  };
}
