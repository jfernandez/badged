{
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  glib,
  pango,
  cairo,
  gdk-pixbuf,
  polkit,
}:

rustPlatform.buildRustPackage {
  pname = "badged";
  version = (lib.importTOML ../Cargo.toml).package.version;

  src = lib.cleanSource ./..;

  # cargoHash (-> fetchCargoVendor, static.crates.io) rather than
  # cargoLock.lockFile (-> importCargoLock, crates.io/api), because the API
  # now 403s nix's `curl/...` User-Agent so importCargoLock can't fetch crates.
  # Regenerate with lib.fakeHash if Cargo.lock changes.
  cargoHash = "sha256-Fu/wdsLtzWnvy9GgOgdSe6afnBDbmZb//IwS/8XHLek=";

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
  ];

  buildInputs = [
    gtk4
    glib
    pango
    cairo
    gdk-pixbuf
    polkit
  ];

  meta = with lib; {
    description = "A polkit authentication agent for Linux window managers";
    homepage = "https://github.com/jfernandez/badged";
    license = licenses.mit;
    platforms = platforms.linux;
    mainProgram = "badged";
  };
}
