--local configs = require("lspconfig.configs")
local nvim_lsp = require("lspconfig")

return {
    lsp = {
        neocmake = {
            cmd = vim.lsp.rpc.connect('127.0.0.1', '9257'),
            root_dir = function(fname)
                return nvim_lsp.util.find_git_ancestor(fname)
            end,
            init_options = {
                AutomaticWorkspaceInit = true,
            },
            on_attach = function(client, bufnr)
                vim.notify("Lsp Start")
                require("cmps.cmp_onattach")(client, bufnr)
            end,
            capabilities = {
                workspace = {
                    didChangeWatchedFiles = {
                        dynamicRegistration = true,
                    },
                },
            }
        }
    }
}
