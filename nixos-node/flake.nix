{
  description = "CSF NixOS Node Configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
  let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    };

    rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
      extensions = [ "rust-src" ];
      targets = [ "x86_64-unknown-linux-gnu" "x86_64-unknown-linux-musl" ];
    };

    gnuPlatform = pkgs.makeRustPlatform {
      cargo = rustToolchain;
      rustc = rustToolchain;
    };

    csfAgentPkg = gnuPlatform.buildRustPackage {
      pname = "csf-agent";
      version = "0.2.2";
      src = ../.;
      cargoLock.lockFile = ../Cargo.lock;
      buildAndTestSubdir = "agent";
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
    };

    csfUpdaterPkg = gnuPlatform.buildRustPackage {
      pname = "csf-updater";
      version = "0.2.2";
      src = ../.;
      cargoLock.lockFile = ../Cargo.lock;
      buildAndTestSubdir = "control-plane/csf-updater";
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [];
      doCheck = false;
    };

    csfDaemonModule = import ./modules/csf-daemon.nix;

    agentSpecialArgs = {
      csf.agentPackage = csfAgentPkg;
      csf.updaterPackage = csfUpdaterPkg;
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
      csf-updater = csfUpdaterPkg;
      default = csfAgentPkg;
      iso = self.nixosConfigurations.iso.config.system.build.isoImage;
    };
  };
}
