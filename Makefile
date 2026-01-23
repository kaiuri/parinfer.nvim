script_dir := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

release_name := parinfer

.PHONY: update
update:
	cargo update --quiet

.PHONY: build
build:
	cargo build --release --quiet
	mkdir -p ${script_dir}/lua
	cp ${script_dir}/target/release/lib${release_name}.so ${script_dir}/lua/${release_name}.so

.PHONY: clean
clean:
	cargo clean --quiet
	rm ${script_dir}/lua/${release_name}.so

.PHONY: test
test:
	cargo test

.PHONY: format
format:
	stylua plugin/parinfer.lua
	cargo fmt --quiet
