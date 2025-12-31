-- print-break.nvim
-- Neovim integration for the print-break Rust debugging crate
-- https://github.com/babinc/print-break
--
-- Installation:
--   Copy this file to: ~/.config/nvim/after/plugin/print-break.lua
--
-- Keybindings (Rust files only):
--   <leader>pb  - Insert print_break!() with word under cursor
--                 If next line already has print_break!, appends to it
--   <leader>pb  - (visual) Wrap selection in print_break!()
--   <leader>pB  - Insert print_break_if!(condition, vars)
--   <leader>pc  - Remove all print_break! from current buffer
--   <leader>pC  - Remove all print_break! from all Rust files in project
--
-- Commands:
--   :PrintBreakClean    - Remove print_break! calls from current buffer
--   :PrintBreakCleanAll - Remove print_break! calls from all Rust files

vim.api.nvim_create_autocmd("FileType", {
  pattern = "rust",
  callback = function(args)
    local bufnr = args.buf

    -- Find the end of the current statement (line ending with ; at appropriate indentation)
    local function find_statement_end(start_row)
      local total_lines = vim.api.nvim_buf_line_count(bufnr)
      local start_line = vim.api.nvim_buf_get_lines(bufnr, start_row - 1, start_row, false)[1]
      local start_indent = #(start_line:match("^(%s*)") or "")

      -- If current line ends with ;, we're done
      if start_line:match(";%s*$") then
        return start_row
      end

      -- Look forward for statement end
      for i = start_row + 1, math.min(start_row + 50, total_lines) do
        local check_line = vim.api.nvim_buf_get_lines(bufnr, i - 1, i, false)[1]
        local check_indent = #(check_line:match("^(%s*)") or "")

        -- Found a line at same/less indentation ending with ;
        if check_line:match(";%s*$") and check_indent <= start_indent then
          return i
        end
        -- Found a line with less indentation (left the block)
        if check_indent < start_indent and check_line:match("%S") then
          return i - 1
        end
      end
      return start_row
    end

    -- <leader>pb in normal mode: Insert print_break!() with word under cursor
    -- If line after statement already has print_break!, append the variable to it
    vim.keymap.set("n", "<leader>pb", function()
      local word = vim.fn.expand("<cword>")
      local row = vim.api.nvim_win_get_cursor(0)[1]
      local total_lines = vim.api.nvim_buf_line_count(bufnr)

      -- Find where the statement ends
      local insert_row = find_statement_end(row)
      local insert_line = vim.api.nvim_buf_get_lines(bufnr, insert_row - 1, insert_row, false)[1]
      local indent = insert_line:match("^(%s*)") or ""

      -- Check if next line has print_break!
      local next_line = ""
      if insert_row < total_lines then
        next_line = vim.api.nvim_buf_get_lines(bufnr, insert_row, insert_row + 1, false)[1] or ""
      end

      if word ~= "" and next_line:match("^%s*print_break!%(") then
        -- Append to existing print_break!
        local new_line = next_line:gsub("print_break!%((.-)%);", function(existing)
          if existing == "" then
            return "print_break!(" .. word .. ");"
          else
            return "print_break!(" .. existing .. ", " .. word .. ");"
          end
        end)
        vim.api.nvim_buf_set_lines(bufnr, insert_row, insert_row + 1, false, { new_line })
      elseif word ~= "" then
        -- Insert new print_break!(word); after statement
        vim.api.nvim_buf_set_lines(bufnr, insert_row, insert_row, false, {
          indent .. "print_break!(" .. word .. ");"
        })
        vim.api.nvim_win_set_cursor(0, { insert_row + 1, 0 })
      else
        -- No word under cursor, insert empty and enter insert mode
        vim.api.nvim_buf_set_lines(bufnr, insert_row, insert_row, false, {
          indent .. "print_break!();"
        })
        vim.api.nvim_win_set_cursor(0, { insert_row + 1, #indent + 12 })
        vim.cmd("startinsert")
      end
    end, { buffer = bufnr, desc = "Insert print_break!()" })

    -- <leader>pb in visual mode: Wrap selection in print_break!()
    vim.keymap.set("v", "<leader>pb", function()
      -- Get the selected text
      vim.cmd('noau normal! "vy')
      local selected = vim.fn.getreg("v")

      -- Clean up the selection (remove newlines, trim)
      selected = selected:gsub("\n", ""):gsub("^%s+", ""):gsub("%s+$", "")

      -- Get current line info
      local row = vim.fn.line("'<")
      local line = vim.api.nvim_buf_get_lines(bufnr, row - 1, row, false)[1]
      local indent = line:match("^(%s*)") or ""

      -- Insert print_break with the variable
      vim.api.nvim_buf_set_lines(bufnr, row, row, false, {
        indent .. "print_break!(" .. selected .. ");"
      })

      -- Exit visual mode and move to inserted line
      vim.cmd("normal! ")
      vim.api.nvim_win_set_cursor(0, { row + 1, 0 })
    end, { buffer = bufnr, desc = "Wrap selection in print_break!()" })

    -- <leader>pB: Insert print_break_if!() for conditional breakpoints
    vim.keymap.set("n", "<leader>pB", function()
      local line = vim.api.nvim_get_current_line()
      local indent = line:match("^(%s*)") or ""
      local row = vim.api.nvim_win_get_cursor(0)[1]

      vim.api.nvim_buf_set_lines(bufnr, row, row, false, {
        indent .. "print_break_if!(, );"
      })

      -- Move cursor to condition position
      vim.api.nvim_win_set_cursor(0, { row + 1, #indent + 16 })
      vim.cmd("startinsert")
    end, { buffer = bufnr, desc = "Insert print_break_if!()" })

    -- <leader>pc: Clean print_break! calls from current buffer
    vim.keymap.set("n", "<leader>pc", "<cmd>PrintBreakClean<cr>",
      { buffer = bufnr, desc = "Clean print_break! from buffer" })

    -- <leader>pC: Clean print_break! calls from all Rust files
    vim.keymap.set("n", "<leader>pC", "<cmd>PrintBreakCleanAll<cr>",
      { buffer = bufnr, desc = "Clean print_break! from all files" })
  end,
})

-- Command to remove all print_break! calls from current buffer
vim.api.nvim_create_user_command("PrintBreakClean", function()
  local bufnr = vim.api.nvim_get_current_buf()
  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local new_lines = {}
  local removed = 0

  for _, line in ipairs(lines) do
    -- Skip lines that are just print_break! or print_break_if! calls
    if not line:match("^%s*print_break!%b();%s*$") and
       not line:match("^%s*print_break_if!%b();%s*$") then
      table.insert(new_lines, line)
    else
      removed = removed + 1
    end
  end

  if removed > 0 then
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, new_lines)
    print("Removed " .. removed .. " print_break! call(s)")
  else
    print("No print_break! calls found")
  end
end, { desc = "Remove all print_break! calls from buffer" })

-- Command to remove print_break from all Rust files in project
vim.api.nvim_create_user_command("PrintBreakCleanAll", function()
  vim.cmd("args **/*.rs")
  vim.cmd("argdo %g/^\\s*print_break\\(_if\\)\\?!(.\\{-});\\s*$/d | update")
  print("Cleaned print_break! from all Rust files")
end, { desc = "Remove all print_break! calls from all Rust files" })
