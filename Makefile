# One entry point for people, agents and CI. Each target runs the commands
# listed in AGENTS.md (Commands), in the order CI's `check` job runs them.
.PHONY: help setup voice-libs lint ui-build test bindings check agents e2e soak dev preview layout

# Rust on Jakub's Mac comes from Homebrew's keg-only rustup.
ifneq ($(wildcard /opt/homebrew/opt/rustup/bin),)
export PATH := /opt/homebrew/opt/rustup/bin:$(PATH)
endif

# The checked sherpa-onnx archives crates/voice links (docs/releasing.md).
# The folder is the one .github/scripts/sherpa-onnx-libs.sh writes to locally.
SHERPA_ONNX_ARCHIVE_DIR ?= $(patsubst %/,%,$(or $(TMPDIR),/tmp))/sherpa-onnx-archives
export SHERPA_ONNX_ARCHIVE_DIR

help: ## List the targets
	@grep -E '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-11s %s\n", $$1, $$2}'

setup: voice-libs ## Install the UI as locked and fetch the voice libraries
	cd app && npm ci

voice-libs: ## Fetch and check the sherpa-onnx libraries (once per machine)
	.github/scripts/sherpa-onnx-libs.sh .

lint: ## ESLint (UI) and clippy (Rust), warnings are errors
	cd app && npm run lint
	cargo clippy --workspace --all-targets -- -D warnings

ui-build: ## Type-check and build the UI (Tauri embeds it, so Rust needs it first)
	cd app && npm run build

test: ui-build ## vitest, then the Rust workspace (also rewrites app/src/bindings)
	cd app && npm test
	cargo test --workspace

bindings: ## Fail if app/src/bindings differs from what the Rust types generate
	@if [ -n "$$(git status --porcelain -- app/src/bindings)" ]; then \
	  git status --short -- app/src/bindings; \
	  echo "app/src/bindings is out of date: commit what cargo test --workspace wrote."; exit 1; \
	fi

check: agents lint test bindings ## Everything CI's check job runs, plus the agent config

agents: ## Generated Codex/Copilot agent files are in sync with .claude/
	python3 scripts/sync_agents.py --check

e2e: ## E2E tests of the real app (Linux; needs tauri-driver, webkit2gtk-driver, xvfb, xdotool)
	cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e

soak: ## 5000 random shows in release mode (SOAK_SEED=<n> replays one)
	SOAK_SHOWS=$${SOAK_SHOWS:-5000} cargo test --release -p show-sim

dev: ## Run the app with hot reload
	cd app && npm run tauri dev

preview: ## UI preview in a browser with a mocked backend (port 1420)
	cd app && npm run dev

layout: ## Layout check of every window (needs `make preview` running and Playwright)
	node .claude/skills/buzzdome-layout/check-layout.mjs --shots .work/layout
