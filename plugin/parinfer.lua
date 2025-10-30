if vim.g.loaded_parinfer ~= nil then
  return
end

if not pcall(require, "parinfer") then
  return vim.notify("parinfer.nvim requires parinfer.so, cd into the plugin and run `make all`")
end

vim.g.loaded_parinfer = true

---@type parinfer_lib
local parinfer = assert(package.loaded.parinfer, "parinfer_lib not loaded")
---@type parinfer_plugin_results
local parinfer_results = {}
---@type parinfer_plugin_state
local parinfer_states = {}

---------- format ----------
---runs parinfer on a buffer
---@param buf integer
local parinfer_run = function(buf)
  local state = parinfer_states[buf] or { vim.bo.filetype }
  state[2] = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  state[3] = vim.api.nvim_win_get_cursor(0)
  -- after the call to parinfer_lib.run
  -- state[2] is now the new lines and state[3] is now the new cursor
  -- state[4] and state[5] are the old state[2] and state[3] respectively
  local result, changes = parinfer.run(state)
  -- changes is nil if nothing needs fixing, which might happen even on successful runs
  -- instead of checking the undotree, we try to undojoin, which throws when outside an undoleaf
  -- it keeps state in sync and has less overhead than running undotree() and :silent! undojoin
  if changes and pcall(vim.api.nvim_exec2, "silent undojoin", { output = false }) then
    vim.api.nvim_buf_set_lines(buf, changes[1], changes[2] + 1, false, table.move(state[2], changes[3] + 1, changes[4] + 1, 1, {}))
    vim.api.nvim_win_set_cursor(0, state[3])
  end
  parinfer_results[buf] = result
  parinfer_states[buf] = state
end

-- decorations
--- parinfer_namespace
local parinfer_namespace = vim.api.nvim_create_namespace("parinfer")
--- parinfer decoration_provider
---@type vim.api.keyset.set_decoration_provider
local parinfer_decoration_provider = {
  on_buf = function(_, bufnr)
    ---@diagnostic disable-next-line: redundant-return-value
    return parinfer_results[bufnr] ~= nil
  end,
  on_win = function(_, winid, bufnr, toprow, botrow)
    local response = parinfer_results[bufnr]
    if response == nil then
      return false
    end
    local ns = parinfer_namespace
    vim.api.nvim_buf_clear_namespace(bufnr, ns, toprow, botrow)
    local paren_trails = response.paren_trails
    if #paren_trails == 0 then return end
    local lines = vim.api.nvim_buf_get_lines(bufnr, toprow, botrow + 1, false)
    ---@type vim.api.keyset.set_extmark
    local extmark_opts = { ephemeral = true, hl_group = "ParinferParenTrail", hl_mode = "combine" }
    for _, paren_trail in ipairs(paren_trails) do
      local line_no = paren_trail[1]
      if line_no >= toprow and line_no <= botrow then
        local line = lines[line_no - toprow + 1]
        local start_x = parinfer.to_bytepos(line, paren_trail[2])
        local offset = start_x - paren_trail[2]
        extmark_opts.end_row = line_no
        extmark_opts.end_col = paren_trail[3] + offset
        vim.api.nvim_buf_set_extmark(bufnr, ns, line_no, start_x, extmark_opts)
      end
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
---@param dx -1|1 # less than 0 means dedent, greater than 0 means indent
local parinfer_shift_indent = function(dx)
  local buf = vim.api.nvim_get_current_buf()
  local response = parinfer_results[buf]
  if response == nil then return end

  local line = vim.api.nvim_get_current_line()
  local indent = select(2, line:find("^%s*"))
  local new_indent = parinfer.shift_indent(response.tab_stops, dx, indent)
  if new_indent == indent then return end
  local new_line = string.gsub(line, "^%s*", string.rep(" ", new_indent))
  vim.api.nvim_set_current_line(new_line)
  local cursor = vim.api.nvim_win_get_cursor(0)
  cursor[2] = new_indent
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
vim.api.nvim_set_keymap("i", "<plug>(parinfer-indent)", "", { noremap = true, callback = function() parinfer_shift_indent(1) end })
vim.api.nvim_set_keymap("i", "<plug>(parinfer-dedent)", "", { noremap = true, callback = function() parinfer_shift_indent(-1) end })
--- commands
vim.api.nvim_create_user_command("ParinferDecorations", parinfer_decorations, { force = true, nargs = "?" })
vim.api.nvim_set_hl(0, "ParinferParenTrail", { link = "NonText" })
--- autocmds
vim.api.nvim_create_autocmd("FileType", {
  pattern = { "clojure", "scheme", "lisp", "racket", "hy", "fennel", "janet", "carp", "wast", "yuck", "dune", "chicken", "query" },
  group = vim.api.nvim_create_augroup("parinfer", { clear = true }),
  callback = parinfer_on_filetype,
})

---
---@class (exact) parinfer_lib.error
---@field name "quote-danger" |"eol-backslash" |"unclosed-quote" |"unclosed-paren" |"unmatched-close-paren" |"unmatched-open-paren" |"leading-close-paren" |"utf8-error" |"json-error" |"panic"
---@field message string
---@field col number
---@field row number
---
---@class (exact) parinfer_lib.editor_state
---@field [1] string
---@field [2] string[]
---@field [3] ([number, number])
---@field private [4]? string[]
---@field private [5]? ([number, number])
---
---@alias parinfer_lib_tab_stop ([number, number, 40|91|123, number])
---
---@class (exact) parinfer_lib.result
---@field tab_stops ([number, number, 40|91|123, number])[]
---@field paren_trails ([number, number, number])[]
---@field error? parinfer_lib.error
---
---@class (exact) parinfer_lib.changes
---@field [1] number # insert start
---@field [2] number # insert stop
---@field [3] number # range start
---@field [4] number # range stop
---
---parinfer results for each buffer
---@class (exact) parinfer_plugin_results
---@field [number]? parinfer_lib.result # parinfer results for each buffer
---
---parinfer state for each buffer
---@class (exact) parinfer_plugin_state
---@field [number]? parinfer_lib.editor_state # parinfer state for each buffer
---
---rust library, must be compiled and on path
---@class (exact) parinfer_lib
---@field run fun(state: parinfer_lib.editor_state): parinfer_lib.result, parinfer_lib.changes?
---@field to_charpos fun(line: string, bytepos: number): number
---@field to_bytepos fun(line: string, charpos: number): number
---@field shift_indent fun(tab_stops: parinfer_lib_tab_stop[], dx: number, x: number): number
