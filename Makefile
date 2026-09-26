.PHONY: test check app install install-linux run

test:
	cargo test

check:
	cargo clippy --all-targets -- -D warnings

run:
	cargo run

app:
	./scripts/build-app.sh

install:
	./scripts/install.sh

install-linux:
	./scripts/install-linux.sh
