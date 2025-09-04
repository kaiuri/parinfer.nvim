---@class parinfer.options
---@field changes?            parinfer.change[]
---@field commentChars?       string[]|string
---@field forceBalance?       boolean
---@field partialResult?      boolean
---@field returnParens?       boolean
---@field cursorLine?         integer
---@field cursorX?            integer
---@field prevCursorLine?     integer
---@field prevCursorX?        integer
---@field selectionStartLine? integer

---@class parinfer.change
---@field lineNo  integer # starting line number of the change
---@field x       integer # starting x of the change
---@field oldText string # original text that was replaced
---@field newText string # new text that replaced the original text

---@class parinfer.error
---@field name    "quote-danger"|"eol-backslash"|"unclosed-quote"|"unclosed-paren"|"unmatched-close-paren"|"unmatched-open-paren"|"leading-close-paren"|"unhandled"
---@field message string                       # is a message describing the error
---@field lineNo  integer                      # is a zero-based line number where the error occurred
---@field x       integer                      # is a zero-based column where the error occurred
---@field extra?  {lineNo: integer,  x: integer} # of open-paren for unmatched-close-paren

---@class (exact) parinfer.tabstop
---@field ch     '('|'['|'{'
---@field lineNo integer
---@field x      integer
---@field argX?  integer

---@class parinfer.ok
---@field success     true                   # indicating if the input was properly formatted enough to create a valid result
---@field text        string                 # is the full text output (if success is false, returns original text unless partialResult is enabled)
---@field cursorX     integer                # is the new, 1-index, cursor column, parinfer may shift it around
---@field cursorLine  integer                # is the new, 1-index, cursor line, rarely ever parinfer will shift it
---@field tabStops    parinfer.tabstop[]
---@field parenTrails parinfer.paren_trail[]
---@field parens?     any[]                  # AST
---@field error nil

---@class parinfer.err
---@field success     false                  # indicating if the input was properly formatted enough to create a valid result
---@field text        string                 # is the full text output (if success is false, returns original text unless partialResult is enabled)
---@field cursorX     integer                # is the new position of the cursor (since parinfer may shift it around)
---@field cursorLine  integer                # is the new position of the cursor (since parinfer may shift it around)
---@field error       parinfer.error         # is an object populated if success is false:
---@field tabStops    nil
---@field parenTrails nil

---@class parinfer.paren_trail
---@field endX   integer
---@field lineNo integer
---@field startX integer

---@alias parinfer_mode 'parenMode'|'indentMode'|'smartMode'

---@type table<parinfer_mode, (fun(text: string, options: parinfer.options): (parinfer.ok|parinfer.err))>
local parinfer = require("parinfer")

---@type { [number]: parinfer.state }
local states = {}

---@param buf number
---@return nil
local attach = function(buf)
  assert(states[buf] == nil, debug.traceback("attach called more than once"))
  local cursor = vim.api.nvim_win_get_cursor(0)
  ---@type parinfer.options
  local options =
    { commentChars = vim.api.nvim_get_option_value("commentstring", { buf = buf }):format(""):match("^.") }
  local result = parinfer.parenMode(table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n"), options)
  if result.success then vim.api.nvim_buf_set_lines(buf, 0, -1, false, vim.split(result.text, "\n")) end
  options.changes = {}
  options.cursorLine = cursor[1]
  options.cursorX = cursor[2] + 1
  options.prevCursorLine = nil
  options.prevCursorX = nil
  options.partialResult = true

  ---@class (exact) parinfer.state
  ---@field textlock boolean
  ---@field disabled boolean
  ---@field options parinfer.options
  ---@field result parinfer.ok|parinfer.err
  states[buf] = {
    ---@type boolean
    textlock = false,
    ---@type boolean
    disabled = false,
    ---@type parinfer.options
    options = options,
    ---@type parinfer.ok|parinfer.err
    result = result,
  }
end

---@type vim.api.keyset.buf_attach
local buf_attach = {
  on_bytes = function(_, bufnr, _, start_row, start_col, start_byte, _, _, old_end_byte, _, _, new_end_byte)
    local state = states[bufnr]
    if state == nil then
      return true -- detach
    elseif state.textlock or state.disabled then
      return
    else
      if start_byte == old_end_byte and start_byte == new_end_byte then return end
      local old_buffer = state.result.text
      local new_buffer = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
      state.options.changes[1] = {
        lineNo = start_row + 1,
        x = start_col + 1,
        oldText = string.sub(old_buffer, start_byte + 1, start_byte + old_end_byte),
        newText = string.sub(new_buffer, start_byte + 1, start_byte + new_end_byte),
      }
      state.result.text = new_buffer
    end
  end,
}

local function autocmd_cursor_moved(ctx)
  local state = states[ctx.buf]
  local cursor = vim.api.nvim_win_get_cursor(0)
  state.options.cursorLine = cursor[1]
  state.options.cursorX = cursor[2] + 1
end

local function autocmd_text_changed(ctx)
  if vim.api.nvim_get_mode().blocking then return end
  if not states[ctx.buf] then return end
  local state = states[ctx.buf]
  if state.disabled or state.textlock or not vim.bo.modifiable or vim.bo.readonly then return end
  do
    local undotree = vim.fn.undotree(ctx.buf)
    if undotree.seq_cur ~= undotree.seq_last then return end
  end
  local cursor = vim.api.nvim_win_get_cursor(0)
  local old_text = table.concat(vim.api.nvim_buf_get_lines(ctx.buf, 0, -1, false), "\n")
  state.options.prevCursorLine = state.options.cursorLine
  state.options.prevCursorX = state.options.cursorX
  state.options.cursorLine = cursor[1]
  state.options.cursorX = cursor[2] + 1
  state.result = parinfer.smartMode(old_text, state.options)
  state.options.changes = {}
  if state.result.success == false then return end
  if state.result.text == old_text then return end
  local new_text = state.result.text
  local buf = ctx.buf
  state.textlock = true
  vim.schedule(function()
    ---@diagnostic disable-next-line: missing-fields
    vim.api.nvim_cmd({ cmd = "undojoin", mods = { emsg_silent = true, silent = true } }, { output = false })
    local lines = vim.split(new_text, "\n")
    local hunks = vim.text.diff(old_text, new_text, { result_type = "indices" }) --[[@as integer[][] ]]
    for _, hunk in ipairs(hunks) do
      local start_a, count_a, start_b, count_b = unpack(hunk, 1, 4)
      local chunks = table.move(lines, start_b, start_b + count_b, 1, {})
      vim.api.nvim_buf_set_lines(buf, start_a - 1, start_a + count_a, false, chunks)
    end
    vim.api.nvim_win_set_cursor(0, { state.result.cursorLine, state.result.cursorX - 1 })
    state.textlock = false
  end)
end

local function autocmd_buf_delete(ctx)
  states[ctx.buf] = nil
end

local api = {}

---@param dx 1|-1
function api.tab(dx)
  local buf = vim.api.nvim_get_current_buf()
  local state = states[buf]
  if state == nil then return end
  local lnum, col = unpack(vim.api.nvim_win_get_cursor(0), 1, 2)
  local next_x = nil
  if state.result.tabStops and vim.api.nvim_get_current_line():find("%S", col + 1) == col + 1 then
    local stops = {}
    for _, tabstop in ipairs(state.result.tabStops) do
      table.insert(stops, (tabstop.x - 1))
      table.insert(stops, tabstop.x + (tabstop.ch == "(" and 1 or 0))
      if tabstop.argX then table.insert(stops, (tabstop.argX - 1)) end
    end
    local left, right = nil, nil
    for _, stop in ipairs(stops) do
      if right then break end
      if col < stop then right = stop end
      if col > stop then left = stop end
    end
    next_x = dx == 1 and right or left
  end
  if not next_x then next_x = math.max(0, col + (dx * 2)) end
  local shift = (next_x - col)
  vim.api.nvim_buf_set_text(
    0,
    (lnum - 1),
    0,
    (lnum - 1),
    ((shift > 0) and 0) or math.abs(shift),
    { string.rep(" ", shift) }
  )
  vim.api.nvim_win_set_cursor(0, { lnum, next_x })
end

function api.toggle()
  local buf = vim.api.nvim_get_current_buf()
  local state = states[buf]
  if not state then return end
  state.disabled = not state.disabled
  vim.api.nvim_buf_set_var(buf, "parinfer", state.disabled and "" or "parinfer")
end

function api.debug()
  local buf = vim.api.nvim_get_current_buf()
  local state = states[buf]
  if not state then return end
  vim.print(state)
end

function api.state(buf)
  buf = buf or vim.api.nvim_get_current_buf()
  local state = states[buf]
  if state and not state.disabled then
    local tabStops = state.result.tabStops
    local parenTrails = state.result.parenTrails
    ---@class parinfer.public_state
    local public_result = { tabStops = tabStops, parenTrails = parenTrails }
    return public_result
  end
end

vim.api.nvim_set_keymap("i", "<plug>(parinfer-tab)", "", {
  noremap = true,
  callback = function()
    api.tab(1)
  end,
})
vim.api.nvim_set_keymap("i", "<plug>(parinfer-backtab)", "", {
  noremap = true,
  callback = function()
    api.tab(-1)
  end,
})
vim.api.nvim_create_autocmd("FileType", {
  pattern = { "clojure", "scheme", "lisp", "racket", "hy", "fennel", "janet", "carp", "wast", "yuck", "dune", "query" },
  group = vim.api.nvim_create_augroup("parinfer", { clear = true }),
  callback = function(ctx)
    if vim.wo.previewwindow then return end
    if vim.bo.buftype ~= "" and not vim.bo.modifiable then return end
    attach(ctx.buf)
    vim.api.nvim_buf_set_var(ctx.buf, "parinfer", "parinfer")
    vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<C-t>", "<plug>(parinfer-tab)", { noremap = true })
    vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<C-d>", "<plug>(parinfer-backtab)", { noremap = true })
    vim.api.nvim_buf_create_user_command(ctx.buf, "ParinferToggle", api.toggle, { force = true })
    vim.api.nvim_buf_attach(ctx.buf, false, buf_attach)
    vim.api.nvim_create_autocmd(
      { "CursorMoved", "CursorMovedI" },
      { group = ctx.group, buffer = ctx.buf, callback = autocmd_cursor_moved }
    )
    vim.api.nvim_create_autocmd(
      { "TextChangedI", "TextChanged" },
      { group = ctx.group, buffer = ctx.buf, callback = autocmd_text_changed }
    )
    vim.api.nvim_create_autocmd("BufDelete", { callback = autocmd_buf_delete, group = ctx.group, buffer = ctx.buf })
  end,
})

return api
