{
  makeRustPlatform,
  rust-bin,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  webkitgtk_6_0,
  glib-networking,
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
  # glib-networking は GIO の TLS バックエンド(HTTPS 接続に必須)。buildInputs に置くと
  # wrapGAppsHook4 が lib/gio/modules を検出し GIO_EXTRA_MODULES に載せる(design.md §15)。
  buildInputs = [
    gtk4
    webkitgtk_6_0
    glib-networking
  ];
}
