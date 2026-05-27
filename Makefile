.PHONY: help up down lint test dev-verify

help:
	@echo "Targets:"
	@echo "  up      Start local infrastructure"
	@echo "  down    Stop local infrastructure"
	@echo "  lint    Run linters when service tooling exists"
	@echo "  test    Run tests when service tooling exists"
	@echo "  dev-verify Run local end-to-end verification"

up:
	docker compose up -d

down:
	docker compose down

lint:
	@echo "No linters configured yet."

test:
	cargo test
	python3 -m py_compile services/ai-runtime/app/main.py
	docker compose config >/dev/null

dev-verify:
	bash scripts/dev-verify.sh
