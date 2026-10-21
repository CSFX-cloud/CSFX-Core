{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    nixos-lima = {
      url = "github:nixos-lima/nixos-lima/v0.2.2";
      flake = false;
    };
    csfx-infra = {
      url = "github:CSFX-cloud/CSFX-Infra";
      flake = false;
    };
  };

  outputs = { nixpkgs, nixos-lima, csfx-infra, ... }:
    let
      versions = import "${csfx-infra}/versions.nix";

      infraModules = map (name: "${csfx-infra}/modules/${name}.nix") [
        "csfx-agent"
        "csfx-cp"
        "csfx-garage"
        "csfx-setup"
        "csfx-cluster"
        "csfx-ingress"
      ];

      mkNode = hostName: nixpkgs.lib.nixosSystem {
        system = "aarch64-linux";
        specialArgs = { inherit versions; };
        modules = infraModules ++ [
          "${nixos-lima}/lima.nix"
          (import ./dev.nix { inherit hostName; })
        ];
      };

      nodeNames = [ "cp1" "cp2" "cp3" "w1" ];
    in
    {
      nixosConfigurations = nixpkgs.lib.genAttrs nodeNames mkNode;
    };
}
