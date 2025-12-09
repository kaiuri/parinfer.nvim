if vim.g.loaded_parinfer ~= nil then
  return
end

if not pcall(require, "parinfer") then
  return vim.notify("parinfer.nvim requires parinfer.so, cd into the plugin and run `make all`")
end

vim.g.loaded_parinfer = true
---@alias ParinferEditorState userdata
---@class ParinferSuggestions
---@field lines string[]
---@field cursor ([number, number])
---@field insert_start number
---@field insert_end number
---@class Parinfer
---@field decorations fun(state: ParinferEditorState, toprow: number, botrow: number): [number, number, number][]
---@field init fun(language: string, lines: string[], cursor: ([number,number])): ParinferEditorState
---@field indent fun(state: ParinferEditorState, dedent?: boolean): string, ([number, number])
---@field refresh fun(state: ParinferEditorState, lines: string[], cursor: ([number,number])): nil
---@field suggestions fun(state: ParinferEditorState): ParinferSuggestions?
local parinfer = assert(package.loaded.parinfer, "parinfer_lib not loaded")
---@type table<integer, ParinferEditorState?>
local states = {}

---------- format ----------
---runs parinfer on a buffer
---@param buf integer
local parinfer_run = function(buf)
  if states[buf] then
    parinfer.refresh(
      states[buf],
      vim.api.nvim_buf_get_lines(buf, 0, -1, false),
      vim.api.nvim_win_get_cursor(0)
    )
  else
    states[buf] = parinfer.init(
      vim.bo.filetype,
      vim.api.nvim_buf_get_lines(buf, 0, -1, false),
      vim.api.nvim_win_get_cursor(0)
    )
  end
  local suggestions = parinfer.suggestions(states[buf])
  if suggestions and pcall(vim.api.nvim_exec2, "silent undojoin", { output = false }) then
    vim.api.nvim_buf_set_lines(buf, suggestions.insert_start, suggestions.insert_end + 1, false, suggestions.lines)
    vim.api.nvim_win_set_cursor(0, suggestions.cursor)
  end
end

-- decorations
--- parinfer_namespace
local parinfer_namespace = vim.api.nvim_create_namespace("parinfer")
--- parinfer decoration_provider
---@type vim.api.keyset.set_decoration_provider
local parinfer_decoration_provider = {
  on_buf = function(_, bufnr)
    ---@diagnostic disable-next-line: redundant-return-value
    return states[bufnr] ~= nil
  end,
  on_win = function(_, winid, bufnr, toprow, botrow)
    local state = states[bufnr]
    if state == nil then return false end
    local ns = parinfer_namespace
    vim.api.nvim_buf_clear_namespace(bufnr, ns, toprow, botrow)
    local paren_trails = parinfer.decorations(state, toprow, botrow)
    ---@type vim.api.keyset.set_extmark
    local extmark_opts = { ephemeral = true, hl_group = "ParinferParenTrail", hl_mode = "combine" }
    for _, paren_trail in ipairs(paren_trails) do
      extmark_opts.end_row = paren_trail[1]
      extmark_opts.end_col = paren_trail[3]
      vim.api.nvim_buf_set_extmark(bufnr, ns, paren_trail[1], paren_trail[2], extmark_opts)
    end
    vim.api.nvim__redraw({ win = winid, valid = false })
  end,
}
--- decorations controller
local parinfer_decorations = function()
  local enabled = vim.g.parinfer_decorations
  vim.api.nvim_set_decoration_provider(parinfer_namespace, (enabled and {}) or parinfer_decoration_provider)
  vim.g.parinfer_decorations = not enabled
end

-- indentation
--- indentation controller
---@param dedent? boolean
local parinfer_shift_indent = function(dedent)
  local buf = vim.api.nvim_get_current_buf()
  local state = states[buf]
  if state == nil then return end
  local line, cursor = parinfer.indent(state, dedent)
  vim.api.nvim_set_current_line(line)
  vim.api.nvim_win_set_cursor(0, cursor)
end


-- event handlers
--- refreshing events
---@param ctx vim.api.keyset.create_autocmd.callback_args
local parinfer_on_editor_changed = function(ctx)
  if vim._getvar("b", ctx.buf, "parinfer_enabled") and vim.bo.modifiable and not vim.bo.readonly then
    parinfer_run(ctx.buf)
  end
end

--- initialization event
--- @param ctx vim.api.keyset.create_autocmd.callback_args
local parinfer_on_filetype = function(ctx)
  if vim.wo.previewwindow or not vim.bo.modifiable or vim.bo.readonly or vim.b.dev_base then
    return
  end

  vim.api.nvim_buf_set_var(ctx.buf, "parinfer_enabled", true)
  vim.api.nvim_exec_autocmds("User", { pattern = "Parinfer", modeline = false })
  vim.schedule(function() parinfer_run(ctx.buf) end) -- schedule so that if <afile> is at `argv`, loading isn't blocked
  vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI", "TextChangedP", "TextChangedI", "TextChanged" }, {
    group = ctx.group,
    buffer = ctx.buf,
    callback = parinfer_on_editor_changed,
  })
end

-- init plugin
--- default mappings
vim.api.nvim_set_keymap("i", "<plug>(parinfer-indent)", "", { noremap = true, callback = function() parinfer_shift_indent() end })
vim.api.nvim_set_keymap("i", "<plug>(parinfer-dedent)", "", { noremap = true, callback = function() parinfer_shift_indent(true) end })
--- commands
vim.api.nvim_create_user_command("ParinferDecorations", parinfer_decorations, { force = true, nargs = "?" })
vim.api.nvim_set_hl(0, "ParinferParenTrail", { link = "NonText" })
--- autocmds
vim.api.nvim_create_autocmd("FileType", {
  pattern = { "clojure", "scheme", "lisp", "racket", "hy", "fennel", "janet", "carp", "wast", "yuck", "dune", "chicken", "query" },
  group = vim.api.nvim_create_augroup("parinfer", { clear = true }),
  callback = parinfer_on_filetype,
})
