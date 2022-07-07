# CMake lsp based on Tower and treesitter

[![Crates.io](https://img.shields.io/crates/v/neocmakelsp.svg)](https://crates.io/crates/neocmakelsp)

It is a CMake lsp based on tower-lsp and treesitter 

## Install

```bash
cargo install neocmakelsp
```

## Setup

```lua
local configs = require("lspconfig.configs")
local nvim_lsp = require("lspconfig")
if not configs.neocmake then
    configs.neocmake = {
        default_config = {
            cmd = { "neocmakelsp" },
            filetypes = { "cmake" },
            root_dir = function(fname)
                return nvim_lsp.util.find_git_ancestor(fname)
            end,
            single_file_support = true,-- suggested
            on_attach = on_attach
        }
    }
    nvim_lsp.neocmake.setup({})
end
```


## Features

* complete
* symbol\_provider
* On hover
* GO TO Definitation

## TODO
* Undefined function check

## Show

### Complete and symbol support
![Show](https://raw.githubusercontent.com/Decodetalkers/utils/master/cmakelsp/demo.gif)

### OnHover
![Show](https://raw.githubusercontent.com/Decodetalkers/utils/master/cmakelsp/onhover.jpg)

### GoToDefinition
![Show](https://raw.githubusercontent.com/Decodetalkers/utils/master/cmakelsp/definition.png)
