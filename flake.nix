{
  description = "A polkit authentication agent for Linux window managers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "badged";
        version = "0.1.0";

        src = pkgs.lib.cleanSource ./.;

        cargoLock.lockFile = ./Cargo.lock;

        nativeBuildInputs = with pkgs; [
          pkg-config
          wrapGAppsHook4
        ];

        buildInputs = with pkgs; [
          gtk4
          dbus
          glib
          pango
          cairo
          gdk-pixbuf
          graphene
          harfbuzz
        ];

        meta = with pkgs.lib; {
          description = "A polkit authentication agent for Linux window managers";
          homepage = "https://github.com/jfernandez/badged";
          license = licenses.mit;
          platforms = platforms.linux;
          mainProgram = "badged";
        };
      };

      overlays.default = final: prev: {
        badged = self.packages.${prev.system}.default;
      };
    };
}
