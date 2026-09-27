.PHONY: test check quality security smells crap crap-baseline release-check hooks app install install-linux run

test:
	cargo test

check:
	cargo clippy --all-targets -- -D warnings

quality:
	./scripts/pre-commit.sh

security:
	osv-scanner scan source --lockfile Cargo.lock
	gitleaks git --redact .

smells:
	uvx --from smells==0.5.0 smells check --path . --policy quality-policy.json --format table --log smells-findings.log --report smells-report.json

crap:
	./scripts/crap-gate.sh

crap-baseline:
	./scripts/crap-gate.sh --update-baseline

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
