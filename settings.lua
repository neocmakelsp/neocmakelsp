--local configs = require("lspconfig.configs")
return {
    lsp = {
        neocmake = {
            cmd = vim.lsp.rpc.connect('127.0.0.1', 9257),
            root_markers = { ".git" },
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
                textDocument = {
                    completion = {
                        completionItem = {
                            snippetSupport = true
                        }
                    }
                }
            },
            init_options = {
                format = {
                    enable = true,

                },
                scan_cmake_in_package = false,
                semantic_token = false,
                use_snippets = false
            }
        }
    }
}
