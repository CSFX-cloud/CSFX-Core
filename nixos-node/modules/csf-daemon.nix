{ config, lib, pkgs, ... }:

let
  cfg = config.services.csf-daemon;
  credentialsFile = "/var/lib/csf-daemon/credentials";
in
{
  options.services.csf-daemon = {
    enable = lib.mkEnableOption "CSF local daemon agent";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The csf-agent package to use.";
    };

    apiGateway = lib.mkOption {
      type = lib.types.str;
      example = "https://gateway.csf.example:8000";
      description = "URL of the CSF API Gateway.";
    };

    registrationToken = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "Cluster-wide bootstrap token (csf-bootstrap.*) or node-specific pre-register token (reg_*). Ignored once the agent is registered.";
    };

    heartbeatInterval = lib.mkOption {
      type = lib.types.ints.positive;
      default = 60;
      description = "Heartbeat interval in seconds.";
    };

    logLevel = lib.mkOption {
      type = lib.types.enum [ "trace" "debug" "info" "warn" "error" ];
      default = "info";
      description = "Log level for the daemon.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.csf-daemon = {
      isSystemUser = true;
      group = "csf-daemon";
      home = "/var/lib/csf-daemon";
      shell = pkgs.shadow;
      description = "CSF daemon service user";
    };

    users.groups.csf-daemon = {};

    systemd.tmpfiles.rules = [
      "d /var/lib/csf-daemon 0700 csf-daemon csf-daemon -"
    ];

    systemd.services.csf-daemon = {
      description = "CSF Local Daemon Agent";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        CSF_GATEWAY_URL = cfg.apiGateway;
        CSF_HEARTBEAT_INTERVAL = toString cfg.heartbeatInterval;
        RUST_LOG = cfg.logLevel;
      } // lib.optionalAttrs (cfg.registrationToken != "") {
        CSF_REGISTRATION_TOKEN = cfg.registrationToken;
      };

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/csf-agent";
        Restart = "always";
        RestartSec = "5s";
        User = "csf-daemon";
        Group = "csf-daemon";
        StateDirectory = "csf-daemon";
        StateDirectoryMode = "0700";
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        RestrictRealtime = true;
        SystemCallFilter = "@system-service";
        ReadWritePaths = [ "/var/lib/csf-daemon" ];
      };
    };
  };
}
