{
  self,
  crane,
}: final: prev: let
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
    luaVersion,
  }: let
    luxLuaCargo = craneLib.crateNameFromCargoToml {src = "${self}/lux-lua";};
    canonicalLuaVersion =
      {
        lua51 = "5.1";
        lua52 = "5.2";
        lua53 = "5.3";
        lua54 = "5.4";
      }
      ."${luaVersion}";
    luaPkg = final.${"lua" + builtins.replaceStrings ["."] ["_"] canonicalLuaVersion};
  in
    craneLib.buildPackage (individualCrateArgs
      // {
        pname = "lux-lua-${canonicalLuaVersion}";
        inherit (luxLuaCargo) version;
        cargoExtraArgs = "-p ${luxLuaCargo.pname} --no-default-features --features ${luaVersion}";

        buildInputs = individualCrateArgs.buildInputs ++ [luaPkg];

        # HACK: For some reason, linking via pkg-config fails on darwin
        env =
          (individualCrateArgs.env or {})
          // final.lib.optionalAttrs final.stdenv.isDarwin {
            LUA_LIB = "${luaPkg}/lib";
            LUA_INCLUDE_DIR = "${luaPkg}/include";
            RUSTFLAGS = "-L ${luaPkg}/lib -llua";
          };

        installPhase = let
          libExt = final.stdenv.hostPlatform.extensions.sharedLibrary;
        in ''
          mkdir -p $out/${canonicalLuaVersion}
          cp target/${buildType}/liblux_lua${libExt} $out/${canonicalLuaVersion}/lux.so
        '';
      });

  lux-lua-all = {buildType ? "release"}:
    with final;
      symlinkJoin {
        name = "lux-lua";
        paths = lib.map (luaVersion: mk-lux-lua {inherit luaVersion buildType;}) [
          "lua51"
          "lua52"
          "lua53"
          "lua54"
        ];
      };

  # can't seem to override the buildType with override or overrideAttrs :(
  mk-lux-cli = {buildType ? "release"}: let
    lux-lua = lux-lua-all {inherit buildType;};
  in
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

        # Instruct Lux to search for the lux-specific shared libraries in the lux-lua derivation
        env.LUX_LIB_DIR = lux-lua;
      });
in {
  lux-cli = mk-lux-cli {};
  lux-cli-debug = mk-lux-cli {buildType = "debug";};

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
