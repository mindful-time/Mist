.PHONY: test check quality security release-check hooks app install install-linux run

test:
	cargo test

check:
	cargo clippy --all-targets -- -D warnings

quality:
	./scripts/pre-commit.sh

security:
	osv-scanner scan source --lockfile Cargo.lock
	gitleaks git --redact .

release-check:
	./scripts/release-check.sh

hooks:
	./scripts/install-hooks.sh

run:
	cargo run

app:
	./scripts/build-app.sh

install:
	./scripts/install.sh

install-linux:
	./scripts/install-linux.sh
