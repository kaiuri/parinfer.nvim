if vim.g.loaded_parinfer ~= nil then return end

vim.g.loaded_parinfer = true

require("parinfer.nvim")
