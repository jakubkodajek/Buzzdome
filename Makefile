# One entry point for people and agents. Each target runs the commands listed
# in AGENTS.md (Commands). `check` runs the steps of build.yml's `check` job
# (UI built before Rust, bindings regenerated from an empty folder, voice
# libraries checked); keep the two in step when either changes.
.PHONY: help setup voice-libs ui-deps lint ui-build test bindings check agents e2e soak dev preview layout

# The steps depend on each other's order (voice libraries before clippy, the
# bindings check after the tests), so `make -j` runs them one at a time.
.NOTPARALLEL:

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

setup: voice-libs ui-deps ## Install the UI as locked and fetch the voice libraries

voice-libs: ## Fetch and check the sherpa-onnx libraries (once per machine)
	.github/scripts/sherpa-onnx-libs.sh .

ui-deps: ## Install the UI dependencies as locked (npm ci), as CI does
	cd app && npm ci

lint: ui-build ## ESLint (UI) and clippy (Rust), warnings are errors
	cd app && npm run lint
	cargo clippy --workspace --all-targets -- -D warnings

ui-build: ## Type-check and build the UI (Tauri embeds it, so Rust needs it first)
	cd app && npm run build

# The tests write the TypeScript bindings (ts-rs). Like CI, they start from
# an empty folder, so a type removed in Rust is noticed too.
test: ui-build ## vitest, then the Rust workspace (also rewrites app/src/bindings)
	cd app && npm test
	rm -r -f app/src/bindings
	cargo test --workspace

bindings: ## Fail if app/src/bindings differs from what the Rust types generate
	@if [ -n "$$(git status --porcelain -- app/src/bindings)" ]; then \
	  git status --short -- app/src/bindings; \
	  echo "app/src/bindings is out of date: commit what cargo test --workspace wrote."; exit 1; \
	fi

check: agents voice-libs ui-deps lint test bindings ## The steps of CI's check job (build.yml), plus the agent config

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
