# ==============================================================================
#  codear — Makefile
# ==============================================================================

CARGO ?= cargo
BIN   := agentcli

# Extra arguments, e.g. `make test ARGS=list_dir` or `make run ARGS="--help"`
ARGS ?=

.DEFAULT_GOAL := help

.PHONY: help build release run test test-verbose check fmt fmt-check lint ci clean install

## —— General ———————————————————————————————————————————————————————————————————

help: ## Show this help
	@awk 'BEGIN {FS = ":.*?## "} \
		/^## —— / { sub(/^## /, ""); printf "\n\033[1m%s\033[0m\n", $$0; next } \
		/^[a-zA-Z_-]+:.*?## / { printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2 }' \
		$(MAKEFILE_LIST)
	@echo

## —— Build & Run ———————————————————————————————————————————————————————————————

build: ## Build the debug binary
	$(CARGO) build

release: ## Build the optimized release binary
	$(CARGO) build --release

run: ## Run the CLI (pass arguments with ARGS="...")
	$(CARGO) run --bin $(BIN) -- $(ARGS)

install: ## Install the binary into ~/.cargo/bin
	$(CARGO) install --path .

## —— Testing ———————————————————————————————————————————————————————————————————

test: ## Run all tests (filter with ARGS=<name>)
	$(CARGO) test $(ARGS)

test-verbose: ## Run all tests and show their stdout/stderr
	$(CARGO) test $(ARGS) -- --nocapture

## —— Code Quality ——————————————————————————————————————————————————————————————

check: ## Type-check without building
	$(CARGO) check --all-targets

fmt: ## Format the code
	$(CARGO) fmt

fmt-check: ## Verify formatting without changing files
	$(CARGO) fmt -- --check

lint: ## Run clippy and treat warnings as errors
	$(CARGO) clippy --all-targets -- -D warnings

ci: fmt-check lint test ## Run formatting check, lint and tests

## —— Housekeeping ——————————————————————————————————————————————————————————————

clean: ## Remove build artifacts
	$(CARGO) clean
