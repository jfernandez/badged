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

  cargoLock.lockFile = ../Cargo.lock;

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
