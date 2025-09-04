# parinfer.nvim 💤

## Setup

What setup? None needed 💅

## API

```lua
print(vim.b[1].parinfer) --> 'parinfer' if enabled for buffer 1, '' otherwise

require('parinfer.nvim').tab(1)   --> or <plug>(parinfer-tab)
require('parinfer.nvim').tab(-1)  --> or <plug>(parinfer-backtab)
require('parinfer.nvim').toggle() --> or :ParinferToggle
require('parinfer.nvim').debug()  --> `vim.print` all of parinfer state for the current buffer

---@class parinfer.public_state
---@field tabStops? { argX?: integer, ch: string, lineNo: integer, x: integer }[]
---@field parenTrails? { endX: integer, lineNo: integer, startX: integer }[]
require('parinfer.nvim').state(buf) --> public_state|nil
```

## FAQ

- statusline? 😢??

```lua
vim.opt.statusline:append("%=%{get(b:,'parinfer','')}") --> b:parinfer == 'parinfer' if parinfer is enabled for the current buffer
```

- How do I highlight parenTrails? 🤩??

```lua
vim.api.nvim_set_decoration_provider(vim.api.nvim_create_namespace("parinfer"), {
  on_buf = function(_buf_, bufnr, tick)
    return require("parinfer.nvim").state(bufnr) ~= nil
  end,
  on_win = function(_win_, winid, bufnr, toprow, botrow)
    local state = require("parinfer.nvim").state(bufnr)
    if state == nil then return false end
    local ns = vim.api.nvim_create_namespace("parinfer")
    vim.api.nvim_buf_clear_namespace(bufnr, ns, toprow, botrow)
    local tabStops = state.tabStops
    local parenTrails = state.parenTrails
    ---@type vim.api.keyset.set_extmark
    local tabstop_opts =
      { virt_text = { { "", "DiffAdd" } }, virt_text_pos = "overlay", strict = false, ephemeral = false }
    for _, tabStop in pairs(tabStops or {}) do
      local row = tabStop.lineNo
      if row >= toprow and row <= botrow then
        tabstop_opts.virt_text[1][1] = tabStop.ch
        vim.api.nvim_buf_set_extmark(bufnr, ns, row, tabStop.x - 1, tabstop_opts)
      end
    end
    ---@type vim.api.keyset.set_extmark
    local trail_opts = { hl_group = "DiffChange", strict = false, ephemeral = false }
    for _, parenTrail in pairs(parenTrails or {}) do
      local row = parenTrail.lineNo - 1
      if row >= toprow and row <= botrow then
        trail_opts.end_row = row
        trail_opts.end_col = parenTrail.endX - 1
        vim.api.nvim_buf_set_extmark(bufnr, ns, row, parenTrail.startX - 1, trail_opts)
      end
    end
  end,
})
```

## Credits

- [parinfer/parinfer.js](https://github.com/parinfer/parinfer.js)
- [shaunlebron/parinfer-codemirror](https://github.com/shaunlebron/parinfer-codemirror)
- [oakmac/parinfer-lua](https://github.com/oakmac/parinfer-lua)
- [oakmac/vscode-parinfer](https://github.com/oakmac/vscode-parinfer)
- [gpanders/nvim-parinfer](https://github.com/gpanders/nvim-parinfer)
- [eraserhd/parinfer-rust](https://github.com/eraserhd/parinfer-rust)

## License

`lua/parinfer.lua` is licensed ISC, see
[oakmac/parinfer-lua](https://github.com/oakmac/parinfer-lua).
