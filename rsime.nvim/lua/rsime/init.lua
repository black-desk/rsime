-- SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
--
-- SPDX-License-Identifier: GPL-3.0-or-later

local ui = require("rsime.ui")

local M = {}

local log_file = io.open("/tmp/rsime-nvim.log", "w")

local function log(fmt, ...)
  if log_file then
    log_file:write(os.date("%H:%M:%S ") .. string.format(fmt, ...) .. "\n")
    log_file:flush()
  end
end

local state = {
  job_id = nil,
  composing = false,
  buf = nil,
  win = nil,
  augroup = nil,
}

local config = {
  bin = "rsime",
  rime_user_data_dir = nil,
  special_keys = {
    ["<CR>"]      = "<CR>",
    ["<BS>"]      = "<BS>",
    ["<Space>"]   = "<Space>",
    ["<Esc>"]     = "<Esc>",
    ["<Tab>"]     = "<Tab>",
    ["<S-Tab>"]   = "<Tab>",
    ["<Up>"]      = "<Up>",
    ["<Down>"]    = "<Down>",
    ["<Left>"]    = "<Left>",
    ["<Right>"]   = "<Right>",
    ["<PageUp>"]  = "<PageUp>",
    ["<PageDown>"] = "<PageDown>",
    ["<C-Space>"] = "<Space>",
  },
}

local number_keys = { "1", "2", "3", "4", "5", "6", "7", "8", "9", "0" }

local function on_response(job_id, data, event)
  if event ~= "stdout" then return end

  -- Neovim delivers data as a list of lines (newlines already stripped).
  -- The last element is "" when output ended with a newline.
  for _, line in ipairs(data) do
    if line == "" then goto continue end

    local ok, resp = pcall(vim.json.decode, line)
    if not ok then goto continue end

    if type(resp.commit) ~= "string"
      or type(resp.preedit) ~= "string"
      or type(resp.candidates) ~= "table"
      or type(resp.highlighted) ~= "number"
    then
      goto continue
    end

    -- safety: skip if not in insert/replace mode
    local mode = vim.fn.mode()
    if mode ~= "i" and mode ~= "R" then
      ui.hide(state)
      state.composing = false
      goto continue
    end

    if resp.commit ~= "" then
      local buf = vim.api.nvim_get_current_buf()
      local r, c = unpack(vim.api.nvim_win_get_cursor(0))
      vim.api.nvim_buf_set_text(buf, r - 1, c, r - 1, c, { resp.commit })
      vim.api.nvim_win_set_cursor(0, { r, c + #resp.commit })
    end

    state.composing = resp.preedit ~= "" or #resp.candidates > 0
    if state.composing then
      ui.show(state, resp)
    else
      ui.hide(state)
    end

    ::continue::
  end
end

local function on_exit(job_id, code, event)
  log("on_exit: job_id=%d code=%d", job_id, code)
  state.job_id = nil
  if code ~= 0 then
    vim.notify(string.format("rsime exited with code %d", code), vim.log.levels.ERROR)
  end
end

local function ensure_job()
  if state.job_id then
    return state.job_id
  end
  log("ensure_job: starting %s stdio", config.bin)
  local cmd = { config.bin, "stdio" }
  local job_opts = {
    stdin = "pipe",
    stdout_buffered = false,
    on_stdout = on_response,
    on_exit = on_exit,
  }
  if config.rime_user_data_dir then
    job_opts.env = { string.format("RIME_USER_DATA_DIR=%s", config.rime_user_data_dir) }
  end
  state.job_id = vim.fn.jobstart(cmd, job_opts)
  log("ensure_job: job_id=%d", state.job_id)
  if state.job_id <= 0 then
    vim.notify("Failed to start rsime", vim.log.levels.ERROR)
    state.job_id = nil
    return nil
  end
  return state.job_id
end

local function send_key(key)
  local job = ensure_job()
  if not job then
    log("send_key: no job, dropping key=%s", vim.inspect(key))
    return
  end
  log("send_key: %s", vim.inspect(key))
  vim.fn.chansend(job, key .. "\n")
end

local function handle_char()
  local ch = vim.v.char
  log("handle_char: vim.v.char=%s", vim.inspect(ch))
  -- Space is a printable character that also appears in special_keys.
  -- When the <Space> keymap returns "<Space>" (non-composing), Neovim
  -- inserts a space which re-triggers InsertCharPre.  If we swallow it
  -- here rsime/RIME may not commit it back, so the space is lost.
  -- During composing, space is handled by the keymap, never by us.
  if ch == " " then return end
  if ch == "1" then return end
  if ch == "2" then return end
  if ch == "3" then return end
  if ch == "4" then return end
  if ch == "5" then return end
  if ch == "6" then return end
  if ch == "7" then return end
  if ch == "8" then return end
  if ch == "9" then return end
  if ch == "0" then return end
  send_key(ch)
  vim.v.char = ""
end

local function handle_special(rsime_key)
  log("handle_special: %s", vim.inspect(rsime_key))
  send_key(rsime_key)
end

local function create_autocmds()
  if state.augroup then return end

  state.augroup = vim.api.nvim_create_augroup("rsime", { clear = true })

  vim.api.nvim_create_autocmd("InsertCharPre", {
    group = state.augroup,
    buffer = 0,
    callback = handle_char,
  })

  vim.api.nvim_create_autocmd("InsertLeave", {
    group = state.augroup,
    buffer = 0,
    callback = function()
      if state.composing then
        send_key("<Esc>")
      end
      ui.hide(state)
      state.composing = false
    end,
  })

  vim.api.nvim_create_autocmd({ "WinLeave", "BufLeave" }, {
    group = state.augroup,
    buffer = 0,
    callback = function()
      ui.hide(state)
    end,
  })
end

local function create_keymaps()
  for vim_key, rsime_key in pairs(config.special_keys) do
    vim.keymap.set("i", vim_key, function()
      if state.composing then
        handle_special(rsime_key)
        return ""
      end
      return vim_key
    end, { expr = true, buffer = true, nowait = true })
  end

  for _, key in ipairs(number_keys) do
    vim.keymap.set("i", key, function()
      if state.composing then
        handle_special(key)
        return ""
      end
      return key
    end, { expr = true, buffer = true, nowait = true })
  end
end

local function delete_keymaps()
  for vim_key, _ in pairs(config.special_keys) do
    pcall(vim.keymap.del, "i", vim_key, { buffer = true })
  end
  for _, key in ipairs(number_keys) do
    pcall(vim.keymap.del, "i", key, { buffer = true })
  end
end

local function remove_autocmds()
  if state.augroup then
    vim.api.nvim_del_augroup_by_id(state.augroup)
    state.augroup = nil
  end
end

function M.activate()
  ensure_job()
  create_autocmds()
  create_keymaps()
end

function M.deactivate()
  if state.composing then
    send_key("<Esc>")
  end
  remove_autocmds()
  delete_keymaps()
  ui.hide(state)
  state.composing = false
end

-- Query whether rsime is currently active (input interception installed).
-- Lets user config / statuslines check the enable state directly.
function M.is_active()
  return state.augroup ~= nil
end

function M.toggle()
  if state.augroup then
    M.deactivate()
  else
    M.activate()
  end
end

function M.setup(opts)
  config = vim.tbl_deep_extend("keep", opts or {}, config)
end

function M.stop()
  M.deactivate()
  if state.job_id then
    vim.fn.jobstop(state.job_id)
    state.job_id = nil
  end
end

return M
