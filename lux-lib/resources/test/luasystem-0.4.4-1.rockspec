local package_name = 'luasystem'
local package_version = '0.4.4'
local rockspec_revision = '1'
local github_account_name = 'lunarmodules'
local github_repo_name = 'luasystem'


package = package_name
version = package_version..'-'..rockspec_revision

source = {
  url = 'git+https://github.com/'..github_account_name..'/'..github_repo_name..'.git',
  branch = (package_version == 'scm') and 'master' or nil,
  tag = (package_version ~= 'scm') and 'v'..package_version or nil,
}

description = {
  summary = 'Platform independent system calls for Lua.',
  detailed = [[
    Adds a Lua API for making platform independent system calls.
  ]],
  license = 'MIT <http://opensource.org/licenses/MIT>',
  homepage = 'https://github.com/'..github_account_name..'/'..github_repo_name,
}

dependencies = {
  'lua >= 5.1',
}

local function make_platform(plat)
  local defines = {
    linux = { },
    unix = { },
    macosx = { },
    win32 = { 'WINVER=0x0600', '_WIN32_WINNT=0x0600' },
    mingw32 = { 'WINVER=0x0600', '_WIN32_WINNT=0x0600' },
  }
  local libraries = {
    linux = { 'rt' },
    unix = { },
    macosx = { },
    win32 = { 'advapi32', 'winmm' },
    mingw32 = { },
  }
  local libdirs = {
    linux = nil,
    unix = nil,
    macosx = nil,
    win32 = nil,
    mingw32 = { },
  }
  return {
    modules = {
      ['system.core'] = {
        sources = {
          'src/core.c',
          'src/compat.c',
          'src/time.c',
          'src/environment.c',
          'src/random.c',
          'src/term.c',
          'src/bitflags.c',
          'src/wcwidth.c',
        },
        defines = defines[plat],
        libraries = libraries[plat],
        libdirs = libdirs[plat],
      },
    },
  }
end

build = {
  type = 'builtin',
  platforms = {
    linux = make_platform('linux'),
    unix = make_platform('unix'),
    macosx = make_platform('macosx'),
    win32 = make_platform('win32'),
    mingw32 = make_platform('mingw32'),
  },
  modules = {
    ['system.init'] = 'system/init.lua',
  },
  copy_directories = {},
}
