{
  description = "CSFX Node — binary builds and server configuration";

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

    platform = pkgs.makeRustPlatform {
      cargo = rustToolchain;
      rustc = rustToolchain;
    };

    csfAgentPkg = platform.buildRustPackage {
      pname = "csfx-agent";
      version = "0.2.2";
      src = ../.;
      cargoLock.lockFile = ../Cargo.lock;
      buildAndTestSubdir = "agent";
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
    };

    csfUpdaterPkg = platform.buildRustPackage {
      pname = "csfx-updater";
      version = "0.2.2";
      src = ../.;
      cargoLock.lockFile = ../Cargo.lock;
      buildAndTestSubdir = "control-plane/csfx-updater";
      nativeBuildInputs = [ pkgs.pkg-config pkgs.protobuf ];
      buildInputs = [];
      doCheck = false;
    };

    versions = import ../CSFX-Infra/versions.nix;

    serverSpecialArgs = {
      csfx.agentPackage = csfAgentPkg;
      csfx.updaterPackage = csfUpdaterPkg;
      inherit versions;
    };
  in
  {
    nixosConfigurations.csfx-server = nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs = serverSpecialArgs;
      modules = [ ./modules/server-configuration.nix ];
    };

    nixosConfigurations.csfx-iso = nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs = serverSpecialArgs;
      modules = [ ./modules/iso-configuration.nix ];
    };

    packages.${system} = {
      csfx-agent = csfAgentPkg;
      csfx-updater = csfUpdaterPkg;
      default = csfAgentPkg;
      iso = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = serverSpecialArgs;
        modules = [ ./modules/iso-configuration.nix ];
      }.config.system.build.isoImage;
    };
  };
}
