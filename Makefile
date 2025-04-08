export PATH := $(HOME)/.cargo/bin:$(PATH)
all:
	cargo build

check:
	cargo test -- --test-threads=1

.PHONY: clean
clean:
	cargo clean

.PHONY: install-deps
install-deps:
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
	export PATH="$$HOME/.cargo/bin:$$PATH"