{ config, lib, pkgs, ... }:

let
  cfg = config.services.csf-daemon;
  tokenFile = "/var/lib/csf-daemon/bootstrap-token";
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
      description = "Static one-time registration token. Leave empty when masterNode.enable = true.";
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

    masterNode = {
      enable = lib.mkEnableOption "automatic self-registration for the master control-plane node";

      adminUsername = lib.mkOption {
        type = lib.types.str;
        default = "admin";
        description = "Admin username used to obtain a registration token.";
      };

      adminPassword = lib.mkOption {
        type = lib.types.str;
        description = "Admin password used to obtain a registration token.";
      };
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

    systemd.services.csf-bootstrap = lib.mkIf cfg.masterNode.enable {
      description = "CSF Master Node Bootstrap (one-time self-registration)";
      after = [ "csf-control-plane.service" "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        User = "root";
      };

      script = ''
        set -euo pipefail

        if [ -f ${credentialsFile} ]; then
          echo "csf-bootstrap: agent already registered, skipping"
          exit 0
        fi

        if [ -f ${tokenFile} ]; then
          echo "csf-bootstrap: token already exists, skipping pre-register"
          exit 0
        fi

        GATEWAY="${cfg.apiGateway}"

        echo "csf-bootstrap: waiting for api-gateway at $GATEWAY"
        for i in $(seq 1 60); do
          if ${pkgs.curl}/bin/curl -sf "$GATEWAY/api/system/health" > /dev/null 2>&1; then
            break
          fi
          sleep 5
        done

        echo "csf-bootstrap: logging in as ${cfg.masterNode.adminUsername}"
        JWT=$(${pkgs.curl}/bin/curl -sf -X POST "$GATEWAY/api/login" \
          -H "Content-Type: application/json" \
          -d '{"username":"${cfg.masterNode.adminUsername}","password":"${cfg.masterNode.adminPassword}"}' \
          | ${pkgs.jq}/bin/jq -r '.token')

        if [ -z "$JWT" ] || [ "$JWT" = "null" ]; then
          echo "csf-bootstrap: login failed"
          exit 1
        fi

        HOSTNAME=$(${pkgs.inetutils}/bin/hostname)

        echo "csf-bootstrap: pre-registering node $HOSTNAME"
        TOKEN=$(${pkgs.curl}/bin/curl -sf -X POST "$GATEWAY/api/registry/admin/agents/pre-register" \
          -H "Authorization: Bearer $JWT" \
          -H "Content-Type: application/json" \
          -d "{\"name\":\"$HOSTNAME\",\"hostname\":\"$HOSTNAME\"}" \
          | ${pkgs.jq}/bin/jq -r '.token')

        if [ -z "$TOKEN" ] || [ "$TOKEN" = "null" ]; then
          echo "csf-bootstrap: pre-registration failed"
          exit 1
        fi

        install -m 600 -o csf-daemon -g csf-daemon /dev/null ${tokenFile}
        echo -n "$TOKEN" > ${tokenFile}
        echo "csf-bootstrap: token written to ${tokenFile}"
      '';
    };

    systemd.services.csf-daemon = {
      description = "CSF Local Daemon Agent";
      after = [ "network-online.target" ]
        ++ lib.optionals cfg.masterNode.enable [ "csf-bootstrap.service" ];
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
        ExecStartPre = lib.mkIf cfg.masterNode.enable (
          "+${pkgs.writeShellScript "csf-daemon-prestart" ''
            set -euo pipefail
            TOKEN_ENV=/var/lib/csf-daemon/token.env
            if [ -f ${tokenFile} ] && [ ! -f ${credentialsFile} ]; then
              echo "CSF_REGISTRATION_TOKEN=$(cat ${tokenFile})" > "$TOKEN_ENV"
              chmod 600 "$TOKEN_ENV"
              chown csf-daemon:csf-daemon "$TOKEN_ENV"
            fi
          ''}"
        );
        EnvironmentFile = lib.mkIf cfg.masterNode.enable "-/var/lib/csf-daemon/token.env";
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
