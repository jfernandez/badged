{
  description = "A polkit authentication agent for Linux window managers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = fn:
        nixpkgs.lib.genAttrs systems (system: fn system nixpkgs.legacyPackages.${system});
    in {
      packages = forAllSystems (_system: pkgs: rec {
        badged = pkgs.callPackage ./nix/package.nix { };
        default = badged;
      });

      devShells = forAllSystems (system: pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.badged ];
          nativeBuildInputs = with pkgs; [
            cargo
            clippy
            pkg-config
            rust-analyzer
            rustc
            rustfmt
          ];
        };
      });

      overlays.default = final: prev: {
        badged = final.callPackage ./nix/package.nix { };
      };

      homeManagerModules.default = import ./nix/hm-module.nix self;

      checks = forAllSystems (system: _pkgs: {
        inherit (self.packages.${system}) badged;
      });
    };
}
