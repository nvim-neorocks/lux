{
  self,
  crane,
}: final: prev: let
  cleanCargoSrc = craneLib.cleanCargoSource self;

  craneLib = crane.mkLib prev;

  luxCliCargo = craneLib.crateNameFromCargoToml {src = "${self}/lux-cli";};

  commonArgs = with final; {
    strictDeps = true;

    nativeBuildInputs = [
      pkg-config
      installShellFiles
    ];

    buildInputs =
      [
        luajit
        openssl
        libgit2
        gnupg
        libgpg-error
        gpgme
      ]
      ++ lib.optionals stdenv.isDarwin [
        darwin.apple_sdk.frameworks.Security
        darwin.apple_sdk.frameworks.SystemConfiguration
      ];

    env = {
      # disable vendored packages
      LIBGIT2_NO_VENDOR = 1;
      LIBSSH2_SYS_USE_PKG_CONFIG = 1;
      LUX_SKIP_IMPURE_TESTS = 1;
    };
  };

  lux-deps = craneLib.buildDepsOnly (commonArgs
    // {
      pname = "lux";
      version = "0.1.0";
      src = cleanCargoSrc;
    });

  individualCrateArgs =
    commonArgs
    // {
      src = cleanCargoSrc;
      cargoArtifacts = lux-deps;
      # NOTE: We disable tests since we run them via cargo-nextest in a separate derivation
      doCheck = false;
    };

  # can't seem to override the buildType with override or overrideAttrs :(
  mk-lux-cli = {buildType ? "release"}:
    craneLib.buildPackage (individualCrateArgs
      // {
        inherit (luxCliCargo) pname version;
        cargoExtraArgs = "-p ${luxCliCargo.pname}";
        cargoArtifacts = lux-deps;

        postBuild = ''
          cargo xtask dist-man
          cargo xtask dist-completions
        '';

        postInstall = ''
          installManPage target/dist/lx.1
          installShellCompletion target/dist/lx.{bash,fish} --zsh target/dist/_lx
        '';

        inherit buildType;

        LUX_LIB_DIR = lux-lua;

        meta.mainProgram = "lx";
      });

  lux-lua =
    let luxLuaCargo = craneLib.crateNameFromCargoToml {src = "${self}/lux-lua";};
    in
    craneLib.buildPackage (individualCrateArgs // {
        inherit (luxLuaCargo) pname version;

        # FIXME: mlua-sys still fails saying it couldn't link the functions properly :(
        nativeBuildInputs = with final; individualCrateArgs.nativeBuildInputs ++ [
          lua51Packages.lua
          lua52Packages.lua
          lua53Packages.lua
          lua54Packages.lua
        ];

        buildCargoCommand = ''
          cargo xtask dist-lua
        '';
      });

  lux-workspace-hack = craneLib.mkCargoDerivation {
    src = cleanCargoSrc;
    pname = "lux-workspace-hack";
    version = "0.1.0";
    cargoArtifacts = null;
    doInstallCargoArtifacts = false;

    buildPhaseCargoCommand = ''
      cargo hakari generate --diff
      cargo hakari manage-deps --dry-run
      cargo hakari verify
    '';

    nativeBuildInputs = with final; [
      cargo-hakari
    ];
  };
in {
  inherit lux-deps lux-workspace-hack;
  lux-cli = mk-lux-cli {};
  lux-cli-debug = mk-lux-cli {buildType = "debug";};

  lux-nextest = craneLib.cargoNextest (commonArgs
    // {
      inherit (luxCliCargo) pname version;
      src = cleanCargoSrc;
      nativeCheckInputs = with final; [
        cacert
        cargo-nextest
        zlib # used for checking external dependencies
        lua
        nix # we use nix-hash in tests
      ];

      preCheck = ''
        export HOME=$(realpath .)
      '';

      cargoArtifacts = lux-deps;
      partitions = 1;
      partitionType = "count";
      cargoNextestExtraArgs = "--no-fail-fast --lib"; # Disable integration tests
      cargoNextestPartitionsExtraArgs = "--no-tests=pass";
    });

  lux-clippy = craneLib.cargoClippy (commonArgs
    // {
      inherit (luxCliCargo) pname version;
      src = cleanCargoSrc;
      cargoArtifacts = lux-deps;
    });
}
