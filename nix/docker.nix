self: final: prev: let
  pkgs = final;
  lux-cli-docker = with pkgs;
    pkgs.dockerTools.buildImage {
      name = "lux";
      tag = "cli";
      config = {
        Cmd = ["${lux-cli}/bin/lux"];
      };
    };
  mk-lux-lua-docker = lua_pkg:
    pkgs.dockerTools.buildImage {
      name = "lux";
      fromImage = lux-cli-docker;
      tag = "${lua_pkg.pname}";
      contents = [pkgs.lux-cli lua_pkg];
      config = {
        Cmd = ["${pkgs.lux-cli}/bin/lux" "--lua" "${lua_pkg.pname}"];
      };
    };
in {
  lux-cli-docker = lux-cli-docker;
  lux-lua51-docker = mk-lux-lua-docker pkgs.lux-lua51;
}
