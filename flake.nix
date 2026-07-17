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
    in
    {
      devShells.default = pkgs.mkShell {
        packages = [
          pkgs.rust-bin.stable.latest.default
	  pkgs.just
        ];
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
