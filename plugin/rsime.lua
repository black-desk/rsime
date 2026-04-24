vim.api.nvim_create_user_command("RsimeEnable", function()
  require("rsime").activate()
end, { desc = "Enable rsime input method" })

vim.api.nvim_create_user_command("RsimeDisable", function()
  require("rsime").deactivate()
end, { desc = "Disable rsime input method" })

vim.api.nvim_create_user_command("RsimeToggle", function()
  require("rsime").toggle()
end, { desc = "Toggle rsime input method" })
