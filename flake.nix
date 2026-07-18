{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    agent-skills.url = "github:Kyure-A/agent-skills-nix";
    anthropic-skills = {
      url = "github:anthropics/skills";
      flake = false;
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
  {
    nixpkgs,
    flake-utils,
    agent-skills,
    anthropic-skills,
    rust-overlay,
    ...
  }:
  flake-utils.lib.eachDefaultSystem (
    system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      agentLib = agent-skills.lib.agent-skills;
      sources = {
        anthropic = {
          path = anthropic-skills;
          subdir = "skills";
        };
      };
      catalog = agentLib.discoverCatalog sources;
      allowlist = agentLib.allowlistFor {
        inherit catalog sources;
        enable = [
          "doc-coauthoring"
          "skill-creator"
        ];
      };
      selection = agentLib.selectSkills {
        inherit catalog allowlist sources;
        skills = { };
      };
      bundle = agentLib.mkBundle { inherit pkgs selection; };
      localTargets = {
        claude = agentLib.defaultLocalTargets.claude // { enable = true; };
      };
      # カバレッジ計測(cargo-llvm-cov)には llvm-tools が要るので拡張を足す。
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "llvm-tools-preview" ];
      };
    in
    {
      devShells.default = pkgs.mkShell {
        packages = [
          rustToolchain
	  pkgs.just
	  pkgs.cargo-llvm-cov
	  # CLAUDE.md 規約 4: coverage が見ない「分岐内の挙動」の未検証を機械検出する。
	  pkgs.cargo-mutants
	  # design.md §2・§15: GTK4 / WebKitGTK 6 のビルドに要るネイティブ依存。
	  pkgs.pkg-config
	  pkgs.gtk4
	  pkgs.webkitgtk_6_0
	  # design.md §15: GIO の TLS バックエンド。HTTPS(検索含む)に必須。
	  pkgs.glib-networking
        ];
        # ホスト環境に依存せず devShell 単体で TLS を効かせる。既存の GIO_EXTRA_MODULES
        # (dconf/gvfs 等)を壊さないよう前置で追記する(design.md §15)。
        shellHook = ''
          export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
      packages.default = pkgs.callPackage ./nix/rust.nix { };
      apps = {
	skills-install = {
	  type = "app";
	  program = "${agentLib.mkLocalInstallScript { inherit pkgs bundle; targets = localTargets; }}/bin/skills-install-local";
	};
      };
    }
  );
}
