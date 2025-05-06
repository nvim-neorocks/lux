{
  self,
  crane,
}: final: prev: let
  lib = final.lib;
  craneLib = crane.mkLib prev;

  cleanCargoSrc = craneLib.cleanCargoSource self;

  luxCliCargo = craneLib.crateNameFromCargoToml {src = "${self}/lux-cli";};

  commonArgs = with final; {
    strictDeps = true;

    nativeBuildInputs = [
      pkg-config
      installShellFiles
    ];

    buildInputs =
      [
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
      buildInputs = commonArgs.buildInputs ++ [final.luajit];
    });

  individualCrateArgs =
    commonArgs
    // {
      src = cleanCargoSrc;
      cargoArtifacts = lux-deps;
      # NOTE: We disable tests since we run them via cargo-nextest in a separate derivation
      doCheck = false;
    };

  mk-lux-lua = {
    buildType ? "release",
    luaPkg,
    isLuaJIT,
  }: let
    luaMajorMinor = lib.take 2 (lib.splitVersion luaPkg.version);
    luaVersionDir =
      if isLuaJIT
      then "jit"
      else lib.concatStringsSep "." luaMajorMinor;
    luaSuffix =
      if isLuaJIT
      then "jit"
      else "${lib.concatStringsSep "" luaMajorMinor}";
    luaFeature = "lua${luaSuffix}";

    luxLuaCargo = craneLib.crateNameFromCargoToml {src = "${self}/lux-lua";};
  in
    craneLib.buildPackage (individualCrateArgs
      // {
        pname = "lux-lua";
        inherit (luxLuaCargo) version;
        cargoExtraArgs = "-p ${luxLuaCargo.pname} --no-default-features --features ${luaFeature}";

        buildInputs = individualCrateArgs.buildInputs ++ [luaPkg];

        # HACK: For some reason, linking via pkg-config fails on darwin
        env =
          (individualCrateArgs.env or {})
          // final.lib.optionalAttrs final.stdenv.isDarwin {
            LUA_LIB = "${luaPkg}/lib";
            LUA_INCLUDE_DIR = "${luaPkg}/include";
            RUSTFLAGS = "-L ${luaPkg}/lib -llua";
          };

        postBuild = ''
          cargo xtask${luaSuffix} dist-lua
        '';

        installPhase = ''
          runHook preInstall
          install -D -v target/dist/${luaVersionDir}/* -t $out/${luaVersionDir}
          install -D -v target/dist/lib/pkgconfig/* -t $out/lib/pkgconfig
          runHook postInstall
        '';
      });

  # can't seem to override the buildType with override or overrideAttrs :(
  mk-lux-cli = {buildType ? "release"}:
    craneLib.buildPackage (individualCrateArgs
      // {
        inherit (luxCliCargo) pname version;
        inherit buildType;

        buildInputs = individualCrateArgs.buildInputs ++ [final.luajit];

        cargoExtraArgs = "-p ${luxCliCargo.pname}";

        postBuild = ''
          cargo xtask dist-man
          cargo xtask dist-completions
        '';

        postInstall = ''
          installManPage target/dist/lx.1
          installShellCompletion target/dist/lx.{bash,fish} --zsh target/dist/_lx
        '';

        meta.mainProgram = "lx";
      });
in {
  lux-cli = mk-lux-cli {};
  lux-cli-debug = mk-lux-cli {buildType = "debug";};
  lux-lua51 = mk-lux-lua {
    luaPkg = final.lua5_1;
    isLuaJIT = false;
  };
  lux-lua52 = mk-lux-lua {
    luaPkg = final.lua5_2;
    isLuaJIT = false;
  };
  lux-lua53 = mk-lux-lua {
    luaPkg = final.lua5_3;
    isLuaJIT = false;
  };
  lux-lua54 = mk-lux-lua {
    luaPkg = final.lua5_3;
    isLuaJIT = false;
  };
  lux-luajit = mk-lux-lua {
    luaPkg = final.luajit;
    isLuaJIT = true;
  };

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

  lux-nextest = craneLib.cargoNextest (commonArgs
    // {
      inherit (luxCliCargo) pname version;
      src = self;

      buildInputs = commonArgs.buildInputs ++ [final.luajit];

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

  lux-nextest-lua = craneLib.cargoNextest (commonArgs
    // {
      pname = "lux-lua";
      version = "0.1.0";
      src = self;
      cargoExtraArgs = "-p lux-lua --features test";
      buildInputs = commonArgs.buildInputs;

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

  lux-taplo = with final;
    craneLib.craneLib.taploFmt {
      inherit (luxCliCargo) pname version;
      src = lib.fileset.toSource {
        root = ../.;
        # Don't format the contents of the autogenerated workspace hack crate
        fileset = lib.fileset.difference ../. ../lux-workspace-hack;
      };
    };

  lux-clippy = craneLib.cargoClippy (commonArgs
    // {
      inherit (luxCliCargo) pname version;
      src = cleanCargoSrc;
      buildInputs = commonArgs.buildInputs ++ [final.luajit];
      cargoArtifacts = lux-deps;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    });
}
