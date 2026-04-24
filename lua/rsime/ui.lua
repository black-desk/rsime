local M = {}

local function get_buf(state)
  if state.buf and vim.api.nvim_buf_is_valid(state.buf) then
    return state.buf
  end
  state.buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_option(state.buf, "buftype", "nofile")
  return state.buf
end

local function render_preedit(preedit)
  if preedit == "" then
    return ""
  end
  return preedit .. "|"
end

local function render_candidates(candidates, highlighted)
  if #candidates == 0 then return "" end

  local parts = {}
  for i, cand in ipairs(candidates) do
    local text = cand.text
    if type(cand.comment) == "string" and cand.comment ~= "" then
      text = text .. cand.comment
    end
    local idx = i % 10
    if i - 1 == highlighted then
      table.insert(parts, "[" .. idx .. "." .. text .. "]")
    else if i - 2 == highlighted then
      table.insert(parts, idx .. "." .. text)
    else
      table.insert(parts, " " .. idx .. "." .. text)
    end end
  end
  local result = table.concat(parts)
  if result:sub(-1) ~= "]" then
    result = result .. " "
  end
  return result
end

function M.show(state, resp)
  local preedit_line = render_preedit(resp.preedit)
  local candidate_line = render_candidates(resp.candidates, resp.highlighted)

  local lines = {}

  if preedit_line ~= "" then
    table.insert(lines, preedit_line)
  end
  if candidate_line ~= "" then
    table.insert(lines, candidate_line)
  end

  if #lines == 0 then
    M.hide(state)
    return
  end

  local width = 0
  for _, l in ipairs(lines) do
    width = math.max(width, vim.api.nvim_strwidth(l))
  end

  local buf = get_buf(state)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)

  -- First time showing: save cursor screen position
  if not state.win or not vim.api.nvim_win_is_valid(state.win) then
    state.anchor_row = vim.fn.winline()  -- directly below cursor line
    state.anchor_col = vim.fn.wincol() - 1    -- 0-based col at cursor
  end

  local win_config = {
    relative = "win",
    win = 0,
    width = width,
    height = #lines,
    row = state.anchor_row,
    col = state.anchor_col,
    style = "minimal",
    border = "rounded",
    focusable = false,
  }

  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_set_config(state.win, win_config)
  else
    state.win = vim.api.nvim_open_win(buf, false, win_config)
  end
end

function M.hide(state)
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_close(state.win, true)
    state.win = nil
  end
  state.anchor_row = nil
  state.anchor_col = nil
end

return M
