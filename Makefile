script_dir := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

release_name := parinfer

.PHONY: all
all:
	@cargo build --release
	@mkdir -p ${script_dir}/lua
	@cp ${script_dir}/target/release/lib${release_name}.so ${script_dir}/lua/${release_name}.so


.PHONY: clean
clean:
	cargo clean
	rm ${script_dir}/lua/${release_name}.so


.PHONY: test
test:
	cargo test
