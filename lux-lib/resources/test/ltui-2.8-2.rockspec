package = "ltui"
version = "2.8-2"
source = {
    url = "git://github.com/tboox/ltui",
    tag = "v2.8"
}
description = {
    detailed = [[
LTUI is a cross-platform terminal ui library based on Lua.

This framework originated from the requirements of graphical menu configuration in [xmake](https://github.com/tboox/xmake).
Similar to the kernel kernel's menuconf to configure the compilation parameters, so using curses and lua to implement a cross-platform character terminal ui library.

Refer to kconfig-frontends for style rendering. Of course, users can customize different ui styles.
]],
    homepage = "https://github.com/tboox/ltui",
    summary = "A cross-platform terminal ui library based on Lua",
    license = "Apache-2.0"
}
dependencies = {
    "lua >= 5.1"
}
build = {
    type = "builtin",
    modules = {
        ["ltui"] = {
            sources = "src/core/curses/curses.c",
            libraries = {"curses"}
        }
    },
    install = {
        lua = {
            ["ltui"] = "src/ltui.lua",
            ["ltui.action"] = "src/ltui/action.lua",
            ["ltui.application"] = "src/ltui/application.lua",
            ["ltui.base.bit"] = "src/ltui/base/bit.lua",
            ["ltui.base.list"] = "src/ltui/base/list.lua",
            ["ltui.base.log"] = "src/ltui/base/log.lua",
            ["ltui.base.os"] = "src/ltui/base/os.lua",
            ["ltui.base.path"] = "src/ltui/base/path.lua",
            ["ltui.base.string"] = "src/ltui/base/string.lua",
            ["ltui.base.table"] = "src/ltui/base/table.lua",
            ["ltui.border"] = "src/ltui/border.lua",
            ["ltui.boxdialog"] = "src/ltui/boxdialog.lua",
            ["ltui.button"] = "src/ltui/button.lua",
            ["ltui.canvas"] = "src/ltui/canvas.lua",
            ["ltui.choicebox"] = "src/ltui/choicebox.lua",
            ["ltui.choicedialog"] = "src/ltui/choicedialog.lua",
            ["ltui.curses"] = "src/ltui/curses.lua",
            ["ltui.desktop"] = "src/ltui/desktop.lua",
            ["ltui.dialog"] = "src/ltui/dialog.lua",
            ["ltui.event"] = "src/ltui/event.lua",
            ["ltui.inputdialog"] = "src/ltui/inputdialog.lua",
            ["ltui.label"] = "src/ltui/label.lua",
            ["ltui.mconfdialog"] = "src/ltui/mconfdialog.lua",
            ["ltui.menubar"] = "src/ltui/menubar.lua",
            ["ltui.menuconf"] = "src/ltui/menuconf.lua",
            ["ltui.object"] = "src/ltui/object.lua",
            ["ltui.panel"] = "src/ltui/panel.lua",
            ["ltui.point"] = "src/ltui/point.lua",
            ["ltui.program"] = "src/ltui/program.lua",
            ["ltui.rect"] = "src/ltui/rect.lua",
            ["ltui.scrollbar"] = "src/ltui/scrollbar.lua",
            ["ltui.statusbar"] = "src/ltui/statusbar.lua",
            ["ltui.textarea"] = "src/ltui/textarea.lua",
            ["ltui.textdialog"] = "src/ltui/textdialog.lua",
            ["ltui.textedit"] = "src/ltui/textedit.lua",
            ["ltui.view"] = "src/ltui/view.lua",
            ["ltui.window"] = "src/ltui/window.lua"
        }
    },
    copy_directories = {
        "tests"
    }
}
