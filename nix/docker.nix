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
  mk-lux-lua-docker = lua_pkg:
    with pkgs; let
      isLuaJIT = lib.strings.hasInfix "jit" lua_pkg.pname;
      luaMajorMinor = lib.take 2 (lib.splitVersion luaPkg.version);
      luaVersionDir =
        if isLuaJIT
        then "jit"
        else lib.concatStringsSep "." luaMajorMinor;
    in
      dockerTools.buildImage {
        name = "lux";
        fromImage = lux-cli-docker;
        tag = "${lua_pkg.pname}";
        copyToRoot = buildEnv {
          name = "${lua_pkg.pname}-root";
          paths = [lux-cli lua_pkg];
          pathsToLink = ["/bin" "/lib"];
        };
        config = {
          Cmd = ["lx" "run"];
        };
        # created = date;
      };
in
  with pkgs; {
    lux-cli-docker = lux-cli-docker;
    lux-lua51-docker = mk-lux-lua-docker pkgs.lux-lua51;
  }
