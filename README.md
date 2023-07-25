# CMake LSP implementation based on Tower and Tree-sitter

[![Crates.io](https://img.shields.io/crates/v/neocmakelsp.svg)](https://crates.io/crates/neocmakelsp)

It is a CMake lsp based on tower-lsp and treesitter 

## Install

```bash
cargo install neocmakelsp
```

## Setup

The config of neocmakelsp is in `nvim-lsp-config`, so just follow `nvim-lsp-config` to setup it

neocmakelsp has two start ways: `stdio` and `Tcp`. `Tcp` is for debug. If you want to help me and debug is , you should start it with `Tcp` way.

### Stdio

```lua
local configs = require("lspconfig.configs")
local nvim_lsp = require("lspconfig")
if not configs.neocmake then
    configs.neocmake = {
        default_config = {
            cmd = { "neocmakelsp", "--stdio" },
            filetypes = { "cmake" },
            root_dir = function(fname)
                return nvim_lsp.util.find_git_ancestor(fname)
            end,
            single_file_support = true,-- suggested
            on_attach = on_attach -- on_attach is the on_attach function you defined
        }
    }
    nvim_lsp.neocmake.setup({})
end
```
### Tcp

```lua
if not configs.neocmake then
    configs.neocmake = {
        default_config = {
            cmd = vim.lsp.rpc.connect('127.0.0.1','9257'),
            filetypes = { "cmake" },
            root_dir = function(fname)
                return nvim_lsp.util.find_git_ancestor(fname)
            end,
            single_file_support = true,-- suggested
            on_attach = on_attach -- on_attach is the on_attach function you defined
        }
    }
    nvim_lsp.neocmake.setup({})
end

```

## Help needed 

new version will not work on mac and windows, so I need your help


## Features

* watchfile
* complete
* symbol\_provider
* On hover
* Format
* GO TO Definitation
	* find\_package
	* include
* Search cli
* Get the project struct
* It is also a cli tool to format

### If you want to use watchfile in neovim, use the nightly one, and set

``` lua
capabilities = {
    workspace = {
        didChangeWatchedFiles = {
            dynamicRegistration = true,
        },
    },
}
```

It will check CMakeCache.txt, and get weather the package is exist

## TODO
* Undefined function check
## Show

### Search
![Search](./images/search.png)

### symbol
![Symbol](./images/ast.png)

### Complete and symbol support
![Complete](./images/findpackage.png)
![CompleteFindpackage](./images/complete.png)

### OnHover
![onHover](./images/onhover.png)

### GoToDefinition
![Show](https://raw.githubusercontent.com/Decodetalkers/utils/master/cmakelsp/definition.png)
![JumpToFile](./images/Jump.png)

### Tree
![TreeShow](images/tree.png)

### Format cli

*Note: When formating files, make sure that your .editorconfig file is in your working directory*

```
format the file

Usage: neocmakelsp {format|--format|-F} [OPTIONS] <FormatPath>...

Arguments:
  <FormatPath>...  file or folder to format

Options:
  -o, --override  override
  -h, --help      Print help
```

It will read .editorconfig file to format files, just set like

```ini
[CMakeLists.txt]
indent_style = space
indent_size = 4
```

If you don't want to format a part, just comment `Not Format Me` before that block.
For example:

```cmake
# Not Format Me
ecm_generate_headers(KCoreAddons_HEADERS
    HEADER_NAMES
        KPluginFactory
        KPluginMetaData
        KStaticPluginHelpers
    REQUIRED_HEADERS KCoreAddons_HEADERS
)
```
