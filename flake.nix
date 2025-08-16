{
  description = "A library and client implementation of luarocks";

  nixConfig = {
    extra-substituters = "https://neorocks.cachix.org";
    extra-trusted-public-keys = "neorocks.cachix.org-1:WqMESxmVTOJX7qoBC54TwrMMoVI1xAM+7yFin8NRfwk=";
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    nixpkgs-cross-overlay = {
      url = "github:alekseysidorov/nixpkgs-cross-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    flake-parts,
    git-hooks,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = builtins.attrNames nixpkgs.legacyPackages;
      perSystem = attrs @ {
        system,
        pkgs,
        ...
      }: let
        pkgs = attrs.pkgs.extend self.overlays.default;
        lib = pkgs.lib;
        git-hooks-check = git-hooks.lib.${system}.run {
          src = self;
          hooks = {
            # NOTE: When adding/removing hooks, make sure
            # to update CONTRIBUTING.md for non-nix users.
            alejandra.enable = true;
            rustfmt.enable = true;
          };
        };
      in {
        packages = with pkgs; {
          default = lux-cli;
          inherit
            lux-cli
            lux-lua51
            lux-lua52
            lux-lua53
            lux-lua54
            lux-luajit
            ;
        };

        devShells = let
          mkDevShell = extra_pkgs:
            pkgs.mkShell {
              name = "lux devShell";
              inherit (git-hooks-check) shellHook;
              buildInputs =
                extra_pkgs
                ++ (with pkgs; [
                  rust-analyzer
                  ra-multiplex
                  cargo-nextest
                  cargo-hakari
                  cargo-insta
                  clippy
                  taplo
                  # Needed for integration test builds
                  pkg-config
                  libxcrypt
                  cmakeMinimal
                  zlib
                  gnum4
                ])
                ++ self.checks.${system}.git-hooks-check.enabledPackages
                ++ (lib.filter (pkg: !(lib.hasPrefix "lua" pkg.name)) pkgs.lux-cli.buildInputs)
                ++ pkgs.lux-cli.nativeBuildInputs;
            };

          mkBuildShell = {
            buildInputs ? [],
            shellHook ? "",
          }:
            pkgs.mkShell {
              name = "lux buildShell";
              buildInputs =
                buildInputs
                ++ pkgs.lux-cli.nativeBuildInputs;
              inherit shellHook;
            };

          mkCrossBuildShell = target: let
            crossSystem = {
              config = target;
              isStatic = true;
              useLLVM = true;
            };
            pkgsCross = import nixpkgs {
              localSystem = system;
              inherit crossSystem;
              overlays = [
                inputs.nixpkgs-cross-overlay.overlays.default
              ];
            };
          in
            mkBuildShell
            {
              buildInputs =
                [pkgsCross.rustCrossHook]
                ++ (lib.filter
                  (pkg: !(lib.hasPrefix "lua" pkg.name))
                  pkgsCross.lux-cli.buildInputs)
                ++ pkgsCross.lux-cli.nativeBuildInputs;
              shellHook = pkgsCross.crossBashPrompt;
            };
        in rec {
          default = lua54;
          lua51 = mkDevShell [pkgs.lua5_1];
          lua52 = mkDevShell [pkgs.lua5_2];
          lua53 = mkDevShell [pkgs.lua5_3];
          lua54 = mkDevShell [pkgs.lua5_4];
          luajit = mkDevShell [pkgs.luajit];
          cd =
            mkBuildShell
            {
              buildInputs =
                (lib.filter
                  (pkg: !(lib.hasPrefix "lua" pkg.name))
                  pkgs.lux-cli.buildInputs)
                ++ pkgs.lux-cli.nativeBuildInputs;
            };
          cd_x86_64-unknown-linux-gnu = cd;
          cd_aarch64-apple-darwin = cd;
          cd_x86_64-unknown-linux-musl =
            mkCrossBuildShell "x86_64-unknown-linux-musl";
          cd_aarch64-unknown-linux-gnu =
            mkCrossBuildShell "aarch64-unknown-linux-gnu";
          cd_aarch64-unknown-linux-musl =
            mkCrossBuildShell "aarch64-unknown-linux-musl";
          cd_x86_64-unknown-freebsd =
            mkCrossBuildShell "x86_64-unknown-freebsd";
          cd_x86_64-apple-darwin =
            mkCrossBuildShell "x86_64-apple-darwin";
        };

        checks = rec {
          default = tests;
          inherit
            git-hooks-check
            ;
          tests = pkgs.lux-nextest;
          lua-tests = pkgs.lux-nextest-lua;
          clippy = pkgs.lux-clippy;
          workspace-hack = pkgs.lux-workspace-hack;
          taplo = pkgs.lux-taplo;
        };
      };
      flake = {
        overlays.default = with inputs; import ./nix/overlay.nix {inherit self crane;};
      };
    };
}
