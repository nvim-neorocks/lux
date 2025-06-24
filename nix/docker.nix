args @ {self, ...}: final: prev: let
  pkgs = final;
  lib = final.lib;
  lux-cli-docker = with pkgs;
    dockerTools.buildImage {
      name = "lux";
      tag = "cli";
      fromImage = dockerTools.pullImage {
        imageName = "alpine";
        finalImageTag = "3.22.0";
        sha256 = "sha256-hXlOPhTQMLR76n69eqHxCyU1KDDRgn1sKtNFV4TVjsw=";
        # Generated with `nix-prefetch-docker --name alpine:3.22.0`
        imageDigest = "sha256:8a1f59ffb675680d47db6337b49d22281a139e9d709335b492be023728e11715";
      };
      copyToRoot = buildEnv {
        name = "lux-cli-root";
        paths = [lux-cli bash];
        pathsToLink = ["/bin"];
      };
      config = {
        Cmd = ["lx"];
      };
      #   created = builtins.substring 0 8 self.lastModifiedDate;
    };
  mk-lux-lua-docker = lux_pkg:
    with pkgs; let
      isFullBuild = lux_pkg.pname == "lux-lua-full";
      lua =
        if isFullBuild
        then lua5_1
        else builtins.elemAt (builtins.filter (pkg: pkg.pname == "lua" || pkg.pname == "luajit") lux_pkg.buildInputs) 0;
      isLuaJIT = lua.pname == "luajit";
      luaVersion =
        if isFullBuild
        then ""
        else if isLuaJIT
        then "jit-${lua.version}-"
        else "${lua.version}-";
    in
      dockerTools.buildImage {
        name = "lux";
        fromImage = lux-cli-docker;
        tag = luaVersion + (lux_pkg.version or lux-cli.version); # 5.1-1.2.3 for versioned builds, 1.2.3 for full builds
        copyToRoot = buildEnv {
          name = "${lux_pkg.pname}-root";
          paths = [lux-cli lux_pkg];
          pathsToLink = ["/bin" "/lib"];
        };
        config = {
          Cmd = ["lx" "run"];
          # docker run -v /path/to/project:/data --rm lux:5.1-1.2.3 run
          WorkingDir = "/data";
          Volumes = {"/data" = {};};
        };
        # created = date;
      };
  lux-lua-full = with pkgs;
    symlinkJoin {
      name = "lux-lua-full";
      pname = "lux-lua-full";
      paths = [
        lux-cli
        lux-lua51
        lux-lua52
        lux-lua53
        lux-lua54
        lux-luajit
      ];
    };
in
  with pkgs; {
    lux-cli-docker = lux-cli-docker;
    lux-lua-docker = mk-lux-lua-docker lux-lua-full;
    lux-lua51-docker = mk-lux-lua-docker lux-lua51;
    lux-lua52-docker = mk-lux-lua-docker lux-lua52;
    lux-lua53-docker = mk-lux-lua-docker lux-lua53;
    lux-lua54-docker = mk-lux-lua-docker lux-lua54;
    lux-luajit-docker = mk-lux-lua-docker lux-luajit;
  }
