self: final: prev: let
  pkgs = final;
  lux-cli-docker = with pkgs;
    dockerTools.buildImage {
      name = "lux";
      tag = "cli";
      copyToRoot = buildEnv {
        name = "lux-cli-root";
        paths = [lux-cli];
        pathsToLink = ["/bin"];
      };
      config = {
        Cmd = ["/bin/lux"];
      };
    };
  mk-lux-lua-docker = lua_pkg:
    with pkgs;
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
          Cmd = ["/bin/lux"];
        };
      };
in {
  lux-cli-docker = lux-cli-docker;
  lux-lua51-docker = mk-lux-lua-docker pkgs.lux-lua51;
}
