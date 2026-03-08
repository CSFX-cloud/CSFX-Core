{
  description = "CSF NixOS Node Configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
  };

  outputs = { self, nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};

    csfAgentPkg = pkgs.rustPlatform.buildRustPackage {
      pname = "csf-agent";
      version = "0.2.2";
      src = ../.;
      cargoLock.lockFile = ../Cargo.lock;
      buildAndTestSubdir = "agent";
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
    };

    csfDaemonModule = import ./modules/csf-daemon.nix;

    agentSpecialArgs = {
      csf.agentPackage = csfAgentPkg;
    };
  in
  {
    nixosConfigurations = {
      iso = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [ ./modules/iso-configuration.nix ];
      };

      csf-node = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = agentSpecialArgs;
        modules = [
          csfDaemonModule
          ./modules/node-configuration.nix
        ];
      };

      csf-server = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = agentSpecialArgs;
        modules = [
          csfDaemonModule
          ./modules/server-configuration.nix
        ];
      };
    };

    nixosModules.csf-daemon = csfDaemonModule;

    packages.${system} = {
      csf-agent = csfAgentPkg;
      default = csfAgentPkg;
      iso = self.nixosConfigurations.iso.config.system.build.isoImage;
    };
  };
}
