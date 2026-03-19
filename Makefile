SHELL := /bin/bash

release_name := parinfer

# Detect OS for library extension
ifeq ($(OS),Windows_NT)
    EXT := dll
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Darwin)
        EXT := dylib
    else
        EXT := so
    endif
endif

# Define paths
LIB_SRC := target/release/lib$(release_name).$(EXT)
LIB_DEST := lua/$(release_name).so

.PHONY: all
all: build

.PHONY: update
update:
	cargo update --quiet

$(LIB_SRC): Cargo.toml $(shell find src -type f 2>/dev/null)
	cargo build --release --quiet

.PHONY: build
build: $(LIB_SRC)
	@mkdir -p "lua"
	cp "$(LIB_SRC)" "$(LIB_DEST)"

.PHONY: clean
clean:
	cargo clean --quiet
	rm -f "$(LIB_DEST)"

.PHONY: test
test:
	cargo test

.PHONY: format
format:
	command -v stylua >/dev/null 2>&1 && stylua "plugin/parinfer.lua" || true
	cargo fmt --quiet
