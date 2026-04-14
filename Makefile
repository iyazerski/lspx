install:
	mkdir -p $(HOME)/.local/bin
	cargo install --path . --locked --root $(HOME)/.local

install-lsp:
	uv tool install ty

test:
	cargo test
