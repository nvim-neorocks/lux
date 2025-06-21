# syntax=aminya/dockerfile-plus
# absolute paths because docker
INCLUDE+ docker/builder.Dockerfile

RUN nix --accept-flake-config build .#lux-lua51

INCLUDE+ docker/common.Dockerfile

ENTRYPOINT [ "/build/bin/lx", "run" ]