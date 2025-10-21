script_dir := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

release_name := libparinfer_nvim.so

.PHONY: all
all:
	@cargo build --release
	@mkdir -p ${script_dir}/lua
	@cp ${script_dir}/target/release/${release_name} ${script_dir}/lua/parinfer_lib.so


.PHONY: clean
clean:
	cargo clean


.PHONY: test
test:
	cargo test
