# Project: parinfer.nvim

`parinfer.nvim` is a Neovim plugin that provides a seamless experience for editing code using [eraserhd/parinfer-rust](https://github.com/eraserhd/parinfer-rust)'s implementation of the Parinfer algorithm.

- Purpose: Provide a seamless integration between the Rust implementation of Parinfer and Neovim using `mlua`.

The project is structured in the following way

- `src/`: Rust source code. The folder is organized as follows:
  - Original implementation code of the Parinfer algorithm found at
    [eraserhd/parinfer-rust](https://github.com/eraserhd/parinfer-rust). The
    implementation is composed by the following files:
    - `src/parinfer.rs`: Original rust implementation of the Parinfer algorithm
      - functions `paren_mode`, `smart_mode` and `process`.
    - `src/changes.rs`: Helper function used only in `src/parinfer.rs`' `process` function.
    - `src/types.rs`: Types used throughout the `src/` folder.
  - Neovim plugin code that uses the original implementation of the Parinfer algorithm previously described.
    - `src/lib.rs`: Entry point of the lua library.
    - `src/integration.rs`: The implementation of the lua library.
    - `src/dialect.rs`: Contains a mechanism for creating input tailored to the specific lisp dialect the processed text is written in.
    - `src/conversion.rs`: Collection of utility functions that convert.
      `src/parinfer.rs`' `process` function into output usable by the Neovim lua runtime.
- `plugin/parinfer.lua`: Neovim plugin entry point, here is where the resulting library is loaded.
- `lua/parinfer.so`: The lua library.
