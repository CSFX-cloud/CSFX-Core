{ hostName }:
{ lib, ... }:
let
  binDir = "/opt/csfx-dev/aarch64-unknown-linux-musl/debug";

  devUnits = {
    csfx-migrate = "csfx-migrate";
    csfx-api-gateway = "api-gateway";
    csfx-registry = "registry";
    csfx-scheduler = "scheduler";
    csfx-volume-manager = "volume-manager";
    csfx-failover-controller = "failover-controller";
    csfx-sdn-controller = "sdn-controller";
    csfx-object-storage = "object-storage";
    csfx-agent = "csfx-agent";
  };
in
{
  networking = {
    inherit hostName;
    firewall.enable = lib.mkForce false;
  };

  users.mutableUsers = lib.mkForce true;

  services = {
    csfx-agent = {
      enable = true;
      gatewayUrl = "https://localhost:8000";
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
      updater.infraRepoMirrorUrl = "";
    };
    csfx-setup = {
      enable = true;
      dataPart = "/dev/vdb";
    };
    csfx-cluster.enable = true;
    csfx-garage.enable = true;
  };

  systemd.services = lib.mkMerge [
    (lib.mapAttrs
      (_: bin: { serviceConfig.ExecStart = lib.mkForce "${binDir}/${bin}"; })
      devUnits)
    {
      csfx-api-gateway.environment.STATIC_DIR = lib.mkForce "/csfx-ui";
      csfx-updater.enable = lib.mkForce false;
      csfx-master-join.serviceConfig.ExecStart = lib.mkForce "${binDir}/csfx-agent --join-master";
    }
  ];
}
