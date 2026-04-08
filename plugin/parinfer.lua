-- Note: This plugin makes use of private api `vim._getvar` for performance reasons as vim.b will create a metatable unless we cache
if vim.g.loaded_parinfer ~= nil then return end

vim.g.loaded_parinfer = true

---@alias cursor ([integer, integer])
---@alias editor_state {[1]: string[], [2]: cursor}
---@alias dialect string
---@alias buffer_state {[1]: dialect, [2]: editor_state, [3]?: editor_state }
---@class (exact) parinfer_error
---@field name "quote-danger" |"eol-backslash" |"unclosed-quote" |"unclosed-paren" |"unmatched-close-paren" |"unmatched-open-paren" |"leading-close-paren" |"utf8-error" |"json-error" |"panic"
---@field message string
---@field col number
---@field row number
---@alias tab_stop ([number, number, 40|91|123, number])
---@class (exact) parinfer_result
---@field tab_stops tab_stop[]
---@field paren_trails ([number, number, number])[]
---@field error? parinfer_error
---@field changes? ([number, number, number, number])

---@type table<integer, parinfer_result> # parinfer results for each buffer
local parinfer_results = {}

---@type table<integer, {[1]: dialect, [2]: editor_state, [3]?: editor_state }>
local parinfer_buffer_states = {}

---rust library, must be compiled and on path
---@class (exact) parinfer_lib
---@field run fun(dialect: dialect, state: editor_state, previous_state: editor_state?): parinfer_result, editor_state
---@field to_charpos fun(line: string, bytepos: number): number
---@field to_bytepos fun(line: string, charpos: number): number
---@field shift_indent fun(tab_stops: tab_stop[], dx: number, x: number): number
local parinfer_lib = require("parinfer")

---@param buf integer
local parinfer_run = function(buf)
  if not (vim.api.nvim_buf_is_loaded(buf) and vim.api.nvim_buf_is_valid(buf)) then
    return
  end
  local changedtick = vim.api.nvim_buf_get_changedtick(buf)
  if vim._getvar("b", buf, "parinfer_changedtick") == changedtick then
    return
  end
  vim._setvar("b", buf, "parinfer_changedtick", changedtick)

  local buffer_state = parinfer_buffer_states[buf]
  if buffer_state == nil then
    buffer_state = {
      vim.api.nvim_get_option_value("filetype", { buf = buf }),
      { vim.api.nvim_buf_get_lines(buf, 0, -1, false), vim.api.nvim_win_get_cursor(0) },
    }
  else
    buffer_state[3][1] = buffer_state[2][1]
    buffer_state[3][2] = buffer_state[2][2]
    buffer_state[2] = { vim.api.nvim_buf_get_lines(buf, 0, -1, false), vim.api.nvim_win_get_cursor(0) }
  end
  local result, new_editor_state = parinfer_lib.run(unpack(buffer_state, 1, 3))
  buffer_state[3] = buffer_state[2]
  buffer_state[2] = new_editor_state
  if result.changes and pcall(vim.api.nvim_exec2, "silent undojoin", { output = false }) then
    local changes = result.changes
    ---@cast changes -?
    local changed_lines = table.move(buffer_state[2][1], changes[3] + 1, changes[4] + 1, 1, {})
    vim.api.nvim_buf_set_lines(buf, changes[1], changes[2] + 1, false, changed_lines)
    vim.api.nvim_win_set_cursor(0, buffer_state[2][2])
  end
  parinfer_results[buf] = result
  parinfer_buffer_states[buf] = buffer_state
end

local parinfer_namespace = vim.api.nvim_create_namespace("parinfer")

---@type vim.api.keyset.set_decoration_provider
local parinfer_decoration_provider = {
  on_buf = function(_, bufnr)
    ---@diagnostic disable-next-line: redundant-return-value
    return parinfer_results[bufnr] ~= nil
  end,
  on_win = function(_, winid, bufnr, toprow, botrow)
    local response = parinfer_results[bufnr]
    if response == nil then return false end
    local ns = parinfer_namespace
    vim.api.nvim_buf_clear_namespace(bufnr, ns, toprow, botrow)
    local paren_trails = response.paren_trails
    if #paren_trails == 0 then return end
    local lines = vim.api.nvim_buf_get_lines(bufnr, toprow, botrow + 1, false)
    ---@type vim.api.keyset.set_extmark
    local extmark_opts = { ephemeral = true, hl_group = "ParinferParenTrail", hl_mode = "combine" }
    local to_bytepos = parinfer_lib.to_bytepos
    for _, paren_trail in ipairs(paren_trails) do
      local line_no = paren_trail[1]
      if line_no >= toprow and line_no <= botrow then
        local line = lines[line_no - toprow + 1]
        local start_x = to_bytepos(line, paren_trail[2])
        local offset = start_x - paren_trail[2]
        extmark_opts.end_row = line_no
        extmark_opts.end_col = paren_trail[3] + offset
        vim.api.nvim_buf_set_extmark(bufnr, ns, line_no, start_x, extmark_opts)
      end
    end
    vim.api.nvim__redraw({ win = winid, valid = false })
  end,
}
local parinfer_decorations = function()
  local enabled = vim.g.parinfer_decorations
  vim.api.nvim_set_decoration_provider(parinfer_namespace, (enabled and {}) or parinfer_decoration_provider)
  vim.g.parinfer_decorations = not enabled
end

---@param dx -1|1 # less than 0 means dedent, greater than 0 means indent
local parinfer_shift_indent = function(dx)
  local buf = vim.api.nvim_get_current_buf()
  local response = parinfer_results[buf]
  if response == nil then return end

  local line = vim.api.nvim_get_current_line()
  local indent = select(2, line:find("^%s*"))
  local new_indent = parinfer_lib.shift_indent(response.tab_stops, dx, indent)
  if new_indent == indent then return end
  local new_line = string.gsub(line, "^%s*", string.rep(" ", new_indent))
  vim.api.nvim_set_current_line(new_line)
  local cursor = vim.api.nvim_win_get_cursor(0)
  cursor[2] = new_indent
  vim.api.nvim_win_set_cursor(0, cursor)
end

---@param ctx vim.api.keyset.create_autocmd.callback_args
local parinfer_on_editor_changed = function(ctx)
  if vim._getvar("b", ctx.buf, "parinfer_enabled") and vim.bo.modifiable and not vim.bo.readonly then
    parinfer_run(ctx.buf)
  end
end

---@param ctx vim.api.keyset.create_autocmd.callback_args
local parinfer_on_filetype = function(ctx)
  if vim.wo.previewwindow or not vim.bo.modifiable or vim.bo.readonly
    or vim._getvar("b", ctx.buf, "dev_base") --> treesitter `InspectTree` buffer
  then
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
  vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<c-t>", "<plug>(parinfer-indent)", { noremap = true })
  vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<c-d>", "<plug>(parinfer-dedent)", { noremap = true })
  vim.api.nvim_buf_create_user_command(ctx.buf, "ParinferToggle", "call setbufvar(bufnr(),'parinfer_enabled', getbufvar(bufnr(),'parinfer_enabled',v:true) ? v:false : v:true)", { force = true })
end

-- init plugin
vim.api.nvim_set_hl(0, "ParinferParenTrail", { link = "NonText" })
vim.api.nvim_set_keymap("i", "<plug>(parinfer-indent)", "", { noremap = true, callback = function() parinfer_shift_indent(1) end })
vim.api.nvim_set_keymap("i", "<plug>(parinfer-dedent)", "", { noremap = true, callback = function() parinfer_shift_indent(-1) end })
vim.api.nvim_create_user_command("ParinferDecorations", parinfer_decorations, { force = true, nargs = "?" })
vim.api.nvim_create_autocmd("FileType", {
  pattern = vim.g.parinfer_filetypes or { "clojure", "scheme", "lisp", "racket", "hy", "fennel", "janet", "carp", "wast", "yuck", "dune", "chicken", "query" },
  group = vim.api.nvim_create_augroup("parinfer", { clear = true }),
  callback = parinfer_on_filetype,
})
