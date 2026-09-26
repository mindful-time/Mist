.PHONY: test check app install run

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

