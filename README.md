# parinfer.nvim 🤺

Easy peasy lemon squeezy out those parenthesaches from your nuggin 🤕

<h2>Demo 🎥</h2>
<div align="center">

https://github.com/user-attachments/assets/9e746a26-a869-4fbc-9c37-481f9687731f

</div>

## Requirements 🧰

- NVIM v0.12.0 with luajit
- Rust toolchain .i.e. `cargo` and friends 🤝
- `make` on your path - if you're on linux and it's not, you might be doing it wrong 👀

## Installation 🔧

```bash
mkdir -p ~/.vim/pack/kaiuri/start
cd ~/.vim/pack/kaiuri/start
git clone https://github.com/kaiuri/parinfer.nvim
make build
```

## Usage 👓

- Keymaps :keyboard:
  - indent the codes 🤜 `<plug>(parinfer-indent)`
  - dedent those codes 🤛 `<plug>(parinfer-dedent)`
- Decorations :nail_care:
  - 🔅 Toggle with `:ParinferDecorations`
  - 🎨 Customize with `hl-ParinferParenTrail`
- Events 🔔
  ```lua
  vim.api.nvim_create_autocmd("User", {
    pattern = "Parinfer",
    callback = function(ctx)
      vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<c-t>", "<plug>(parinfer-indent)", { noremap = true })
      vim.api.nvim_buf_set_keymap(ctx.buf, "i", "<c-d>", "<plug>(parinfer-dedent)", { noremap = true })
    end,
  })
  ```
- Disable 🙅? Okay 😇
  ```vim
  " locally, disable parinfer for the current buffer
  let b:parinfer_enabled = v:false
  " or just be done with it and not even load it
  let g:loaded_parinfer = v:true " any value will do
  ```

## Credits‼

- [Jason Felice](https://github.com/eraserhd/parinfer-rust): The implementation is his. Due credits and license at the head of copypasta code in `src/{parinfer,types,changes}.rs`. 🙏
- The rest was written by yours truly 🤦

## Licence 📚

> See [LICENSE](./LICENSE)

### Motivation 💡

Well... I just wanted some more candy and speed for parensurfing... 🏄
