rockspec_format = "3.0"
package = "Luyoga"
version = "1.3-3"
source = {
   url = "git://github.com/SkyLeite/luyoga",
   tag = "v1.3-3"
}
description = {
   summary = "Lua bindings for facebook/yoga, a render-agnostic layouting engine",
   detailed = [[
      Lua bindings for the facebook/yoga layout library.
   ]],
   homepage = "https://skyleite.github.io/luyoga/",
   license = "LGPLv2.1"
}
dependencies = {
   "lua >= 5.1"
}
build = {
    type = "builtin",
    modules = {
        ["luyoga"] = "luyoga/init.lua",
        ["luyoga.layout"] = "luyoga/layout.lua",
        ["luyoga.enums"] = "luyoga/enums.lua",
        ["luyoga.node"] = "luyoga/node.lua",
        ["luyoga.style"] = "luyoga/style.lua",
        ["luyoga.util"] = "luyoga/util.lua",
        ["luyoga.value"] = "luyoga/value.lua",
    },
    install = {
      lib = {
        "dist/libyogacore.so",
        "dist/libyogacore.dylib",
        "dist/Yoga.h",
      },
    }
}