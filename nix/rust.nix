{
  makeRustPlatform,
  rust-bin,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  webkitgtk_6_0,
}:
let
  toolchain = rust-bin.stable.latest.default;
  rustPlatform = makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };
in
rustPlatform.buildRustPackage {
  pname = "owl";
  version = "0.1.0";

  src = ../.;
  cargoLock.lockFile = ../Cargo.lock;

  # design.md §15: pkg-config でネイティブライブラリを解決し、wrapGAppsHook4 で
  # GSettings スキーマ・GLib 環境をラップする。
  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
  ];
  buildInputs = [
    gtk4
    webkitgtk_6_0
  ];
}
