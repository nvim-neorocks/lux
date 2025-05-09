# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## `lux-cli` - [0.3.15](https://github.com/nvim-neorocks/lux/compare/v0.3.14...v0.3.15) - 2025-05-09

### Added
- *(cli)* nicer error messages

### Fixed
- *(cli)* rough UX on luajit

### Other
- *(deps)* bump tokio from 1.44.0 to 1.45.0 ([#659](https://github.com/nvim-neorocks/lux/pull/659))
- add git dependencies to comparison table

## `lux-lib` - [0.3.12](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.11...lux-lib-v0.3.12) - 2025-05-09

### Added
- more Lua coverage + Lua tests

### Fixed
- *(cli)* rough UX on luajit

### Other
- *(deps)* bump tokio from 1.44.0 to 1.45.0 ([#659](https://github.com/nvim-neorocks/lux/pull/659))
- add git dependencies to comparison table
- *(deps)* bump luajit-src from 210.5.11+97813fb to 210.5.12+a4f56a4 ([#656](https://github.com/nvim-neorocks/lux/pull/656))

## `lux-cli` - [0.3.14](https://github.com/nvim-neorocks/lux/compare/v0.3.13...v0.3.14) - 2025-05-01

### Added
- git dependencies for local projects ([#644](https://github.com/nvim-neorocks/lux/pull/644))
- *(lib/install)* support installing from alternate sources ([#624](https://github.com/nvim-neorocks/lux/pull/624))

### Fixed
- *(build)* dependencies added as install tree entrypoints ([#651](https://github.com/nvim-neorocks/lux/pull/651))
- *(build)* transitive dependencies added as dependencies of main package

### Other
- refactor!(lux-lib): builder for `PackageInstallSpec` ([#629](https://github.com/nvim-neorocks/lux/pull/629))

## `lux-lib` - [0.3.11](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.10...lux-lib-v0.3.11) - 2025-05-01

### Added
- git dependencies for local projects ([#644](https://github.com/nvim-neorocks/lux/pull/644))
- *(lib/install)* support installing from alternate sources ([#624](https://github.com/nvim-neorocks/lux/pull/624))

### Fixed
- *(build)* dependencies added as install tree entrypoints ([#651](https://github.com/nvim-neorocks/lux/pull/651))
- *(build)* unpacking tar archive can panic ([#649](https://github.com/nvim-neorocks/lux/pull/649))

### Other
- refactor!(lux-lib): builder for `PackageInstallSpec` ([#629](https://github.com/nvim-neorocks/lux/pull/629))

## `lux-cli` - [0.3.13](https://github.com/nvim-neorocks/lux/compare/v0.3.12...v0.3.13) - 2025-04-29

### Other
- update Cargo.lock dependencies

## `lux-lib` - [0.3.10](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.9...lux-lib-v0.3.10) - 2025-04-29

### Added
- *(lux-lib)* more lenient dev version parsing ([#623](https://github.com/nvim-neorocks/lux/pull/623))

### Fixed
- parse versions without a contraint prefix as == ([#640](https://github.com/nvim-neorocks/lux/pull/640))

### Other
- *(deps)* bump insta from 1.42.0 to 1.43.0 ([#642](https://github.com/nvim-neorocks/lux/pull/642))

## `lux-cli` - [0.3.12](https://github.com/nvim-neorocks/lux/compare/v0.3.11...v0.3.12) - 2025-04-27

### Fixed
- *(cli)* suggest `--no-lock` instead of `--ignore-lockfile`

## `lux-cli` - [0.3.11](https://github.com/nvim-neorocks/lux/compare/v0.3.10...v0.3.11) - 2025-04-27

### Fixed
- conflicting external dependency spec parse error ([#632](https://github.com/nvim-neorocks/lux/pull/632))

## `lux-lib` - [0.3.9](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.8...lux-lib-v0.3.9) - 2025-04-27

### Fixed
- conflicting external dependency spec parse error ([#632](https://github.com/nvim-neorocks/lux/pull/632))

## `lux-cli` - [0.3.10](https://github.com/nvim-neorocks/lux/compare/v0.3.9...v0.3.10) - 2025-04-23

### Other
- *(deps)* bump stylua from 2.0.2 to 2.1.0 ([#621](https://github.com/nvim-neorocks/lux/pull/621))

## `lux-cli` - [0.3.9](https://github.com/nvim-neorocks/lux/compare/v0.3.8...v0.3.9) - 2025-04-22

### Other
- *(deps)* bump stylua from 2.0.0 to 2.0.2 ([#619](https://github.com/nvim-neorocks/lux/pull/619))

## `lux-cli` - [0.3.8](https://github.com/nvim-neorocks/lux/compare/v0.3.7...v0.3.8) - 2025-04-21

### Added
- windows msvc toolchain support ([#501](https://github.com/nvim-neorocks/lux/pull/501))
- `lx generate-rockspec`

### Fixed
- lockfile entries removed after `lx add` ([#617](https://github.com/nvim-neorocks/lux/pull/617))

## `lux-lib` - [0.3.8](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.7...lux-lib-v0.3.8) - 2025-04-21

### Added
- windows msvc toolchain support ([#501](https://github.com/nvim-neorocks/lux/pull/501))

### Fixed
- *(manifest)* re-download if corrupted

### Other
- update flake.lock ([#615](https://github.com/nvim-neorocks/lux/pull/615))

## `lux-cli` - [0.3.7](https://github.com/nvim-neorocks/lux/compare/v0.3.6...v0.3.7) - 2025-04-16

### Other
- updated the following local packages: lux-lib

## `lux-cli` - [0.3.6](https://github.com/nvim-neorocks/lux/compare/v0.3.5...v0.3.6) - 2025-04-14

### Other
- use compilation target to get platform identifier ([#597](https://github.com/nvim-neorocks/lux/pull/597))

## `lux-lib` - [0.3.6](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.5...lux-lib-v0.3.6) - 2025-04-14

### Fixed
- *(pack)* regression in manifest creation ([#599](https://github.com/nvim-neorocks/lux/pull/599))

### Other
- use compilation target to get platform identifier ([#597](https://github.com/nvim-neorocks/lux/pull/597))

## `lux-cli` - [0.3.5](https://github.com/nvim-neorocks/lux/compare/v0.3.4...v0.3.5) - 2025-04-14

### Fixed
- *(cli/install-rockspec)* ensure luarocks is installed
- *(build)* wrap binaries ([#583](https://github.com/nvim-neorocks/lux/pull/583))

## `lux-lib` - [0.3.5](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.4...lux-lib-v0.3.5) - 2025-04-14

### Added
- better dev version parsing
- better variable expansion + error on missing variables

### Fixed
- install pre-packaged luarocks on windows ([#584](https://github.com/nvim-neorocks/lux/pull/584))
- *(build)* wrap binaries ([#583](https://github.com/nvim-neorocks/lux/pull/583))

### Other
- *(deps)* bump bon from 3.5.0 to 3.6.0 ([#586](https://github.com/nvim-neorocks/lux/pull/586))

## `lux-cli` - [0.3.4](https://github.com/nvim-neorocks/lux/compare/v0.3.3...v0.3.4) - 2025-04-13

### Other
- updated the following local packages: lux-lib
## `lux-cli` - [0.3.3](https://github.com/nvim-neorocks/lux/compare/v0.3.2...v0.3.3) - 2025-04-11

### Other
- updated the following local packages: lux-lib
## `lux-cli` - [0.3.2](https://github.com/nvim-neorocks/lux/compare/v0.3.1...v0.3.2) - 2025-04-10

### Other
- updated the following local packages: lux-lib
## `lux-cli` - [0.3.1](https://github.com/nvim-neorocks/lux/compare/v0.3.0...v0.3.1) - 2025-04-10

### Other
- update Cargo.lock dependencies
## `lux-lib` - [0.3.1](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.3.0...lux-lib-v0.3.1) - 2025-04-10

### Fixed
- `[run]` field overwritten by `extra.rockspec` ([#566](https://github.com/nvim-neorocks/lux/pull/566))
- unsupported off-spec `install.bin` array field

## `lux-cli` - [0.3.0](https://github.com/nvim-neorocks/lux/compare/v0.2.4...v0.3.0) - 2025-04-08

### Added
- *(debug project)* flag to list included files ([#556](https://github.com/nvim-neorocks/lux/pull/556))

### Fixed
- [**breaking**] incompatible generated rockspec dependencies

### Other
- make `lx debug`'s description more obvious

## `lux-lib` - [0.3.0](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.2.3...lux-lib-v0.3.0) - 2025-04-08

### Added
- *(debug project)* flag to list included files ([#556](https://github.com/nvim-neorocks/lux/pull/556))

### Fixed
- *(build)* properly handle legacy rockspecs ([#557](https://github.com/nvim-neorocks/lux/pull/557))
- [**breaking**] incompatible generated rockspec dependencies

## `lux-cli` - [0.2.4](https://github.com/nvim-neorocks/lux/compare/v0.2.3...v0.2.4) - 2025-04-08

### Fixed
- *(help)* remove [UNIMPLEMENTED] from `lx doc` help

## `lux-lib` - [0.2.3](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.2.2...lux-lib-v0.2.3) - 2025-04-08

### Fixed
- *(rockspec)* support undocumented string/array duality

## `lux-cli` - [0.2.3](https://github.com/nvim-neorocks/lux/compare/v0.2.2...v0.2.3) - 2025-04-07

### Added
- *(build)* flag to build only dependencies

### Fixed
- fix!(sync): lock constraint changes when syncing with project lockfile
- *(build)* project not added to lockfile

## `lux-lib` - [0.2.2](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.2.1...lux-lib-v0.2.2) - 2025-04-07

### Fixed
- fix!(sync): lock constraint changes when syncing with project lockfile

## `lux-cli` - [0.2.2](https://github.com/nvim-neorocks/lux/compare/v0.2.1...v0.2.2) - 2025-04-07

### Other
- updated the following local packages: lux-lib

## `lux-cli` - [0.2.1](https://github.com/nvim-neorocks/lux/compare/lux-cli-v0.2.0...lux-cli-v0.2.1) - 2025-04-06

### Other
- add `repository` for `lux-cli` so that `cargo binstall` works

## `lux-cli` - [0.2.0](https://github.com/nvim-neorocks/lux/compare/lux-cli-v0.1.0...lux-cli-v0.2.0) - 2025-04-06

### Added
- implicitly propagate environment variables to subprocesses
- enable vim mode for `lx new` selections
- `lx run` command
- *(`lx new`)* create `src` directory automatically
- *(pin)* operate on lux.toml if in a project ([#486](https://github.com/nvim-neorocks/lux/pull/486))
- build project on `lx lua` ([#485](https://github.com/nvim-neorocks/lux/pull/485))
- [**breaking**] allow overriding `etc` tree ([#457](https://github.com/nvim-neorocks/lux/pull/457))
- feat!(toml): `opt` and `pin` fields ([#456](https://github.com/nvim-neorocks/lux/pull/456))
- [**breaking**] optional packages ([#453](https://github.com/nvim-neorocks/lux/pull/453))
- `lux.loader`
- compute hashes for rockspecs dynamically
- *(update)* `--toml` flag to upgrade packages in lux.toml ([#449](https://github.com/nvim-neorocks/lux/pull/449))
- *(remove)* operate on projects ([#448](https://github.com/nvim-neorocks/lux/pull/448))
- *(update)* take an optional list of packages ([#446](https://github.com/nvim-neorocks/lux/pull/446))
- feat!(cli): remove `sync` command
- *(update)* operate on lux.toml and lux.lock if in a project ([#428](https://github.com/nvim-neorocks/lux/pull/428))

### Fixed
- use compilation target to get platform identifier ([#512](https://github.com/nvim-neorocks/lux/pull/512))
- `lx run` does not rebuild the project
- *(`lx new`)* don't search parents for existing project ([#493](https://github.com/nvim-neorocks/lux/pull/493))
- `no such file or directory` when running `lx fmt`
- *(uninstall)* properly handle dependencies

### Other
- turn `run_lua` into an operation
- [**breaking**] rename `lx run` to `lx exec`
- *(deps)* bump octocrab from 0.43.0 to 0.44.0 ([#499](https://github.com/nvim-neorocks/lux/pull/499))
- *(build)* add case for local project with no source ([#490](https://github.com/nvim-neorocks/lux/pull/490))
- inconsistent naming in `lx debug project`
- refactor!(toml): extract `LuaDependency` type ([#454](https://github.com/nvim-neorocks/lux/pull/454))
- prepare flake for new build sequence
- *(deps)* bump tokio from 1.43.0 to 1.44.0 ([#461](https://github.com/nvim-neorocks/lux/pull/461))
- [**breaking**] introduce `LocalLuaRockspec` and `RemoteLuaRockspec`
- [**breaking**] allow building of local rockspecs
- [**breaking**] break apart `ProjectToml` into `LocalProjectToml` and `RemoteProjectToml`
- [**breaking**] break rockspec apart into `LocalRockspec` and `RemoteRockspec`

## `lux-lib` - [0.2.0](https://github.com/nvim-neorocks/lux/compare/lux-lib-v0.1.0...lux-lib-v0.2.0) - 2025-04-06

### Added
- implicitly propagate environment variables to subprocesses
- `lx run` command
- add `operations::run`
- *(`lux.toml`)* add `[run]` support
- *(pin)* operate on lux.toml if in a project ([#486](https://github.com/nvim-neorocks/lux/pull/486))
- *(build)* respect ignore files when copying source ([#495](https://github.com/nvim-neorocks/lux/pull/495))
- [**breaking**] allow overriding `etc` tree ([#457](https://github.com/nvim-neorocks/lux/pull/457))
- feat!(toml): `opt` and `pin` fields ([#456](https://github.com/nvim-neorocks/lux/pull/456))
- [**breaking**] optional packages ([#453](https://github.com/nvim-neorocks/lux/pull/453))
- `lux.loader`
- Lua API
- *(build)* treesitter-parser build backend ([#452](https://github.com/nvim-neorocks/lux/pull/452))
- compute hashes for rockspecs dynamically
- *(update)* `--toml` flag to upgrade packages in lux.toml ([#449](https://github.com/nvim-neorocks/lux/pull/449))
- *(remove)* operate on projects ([#448](https://github.com/nvim-neorocks/lux/pull/448))
- *(update)* take an optional list of packages ([#446](https://github.com/nvim-neorocks/lux/pull/446))
- allow `--tree` to override project tree ([#432](https://github.com/nvim-neorocks/lux/pull/432))
- *(update)* operate on lux.toml and lux.lock if in a project ([#428](https://github.com/nvim-neorocks/lux/pull/428))

### Fixed
- use compilation target to get platform identifier ([#512](https://github.com/nvim-neorocks/lux/pull/512))
- do not include `lua` as part of dependencies in TOML rockspecs
- map between luarocks and semver versions ([#483](https://github.com/nvim-neorocks/lux/pull/483))
- *(build)* don't fall back to `.src.rock` for local sources ([#494](https://github.com/nvim-neorocks/lux/pull/494))
- *(`lx new`)* don't search parents for existing project ([#493](https://github.com/nvim-neorocks/lux/pull/493))
- disallow `lua` in `dependencies` field
- *(uninstall)* properly handle dependencies
- *(build)* copy_directories into etc subdirectories ([#462](https://github.com/nvim-neorocks/lux/pull/462))
- minimize extraneous compiler output

### Other
- *(deps)* bump zip from 2.5.0 to 2.6.0 ([#514](https://github.com/nvim-neorocks/lux/pull/514))
- turn `run_lua` into an operation
- [**breaking**] rename `lx run` to `lx exec`
- *(deps)* bump zip from 2.4.1 to 2.5.0 ([#492](https://github.com/nvim-neorocks/lux/pull/492))
- *(deps)* bump zip from 2.3.0 to 2.4.1
- refactor!(toml): extract `LuaDependency` type ([#454](https://github.com/nvim-neorocks/lux/pull/454))
- *(lockfile)* hide unnecessarily public structs/methods
- [**breaking**] remove `lua` cargo feature
- *(nix)* fix `nix flake check`
- prepare flake for new build sequence
- [**breaking**] name all lockfiles `lux.lock`
- *(deps)* bump zip from 2.2.0 to 2.3.0 ([#470](https://github.com/nvim-neorocks/lux/pull/470))
- *(deps)* bump bon from 3.4.0 to 3.5.0 ([#469](https://github.com/nvim-neorocks/lux/pull/469))
- *(deps)* bump tokio from 1.43.0 to 1.44.0 ([#461](https://github.com/nvim-neorocks/lux/pull/461))
- *(deps)* bump bon from 3.3.2 to 3.4.0 ([#455](https://github.com/nvim-neorocks/lux/pull/455))
- [**breaking**] introduce `LocalLuaRockspec` and `RemoteLuaRockspec`
- *(`rocks pack`)* [**breaking**] disallow paths to `lux.toml` files
- [**breaking**] allow building of local rockspecs
- [**breaking**] break apart `ProjectToml` into `LocalProjectToml` and `RemoteProjectToml`
- [**breaking**] break rockspec apart into `LocalRockspec` and `RemoteRockspec`
- use crane for clippy and rustfmt checks ([#450](https://github.com/nvim-neorocks/lux/pull/450))
- *(deps)* bump flate2 from 1.0.35 to 1.1.0 ([#438](https://github.com/nvim-neorocks/lux/pull/438))
