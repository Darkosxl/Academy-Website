GAME ?= ls20
GAME_SECONDS ?= 45
STEPS ?= 50
SUBMISSION_REPO ?= services/benchmark-node/adapters/live_submission

.PHONY: help benchmark-venv game game-engine harness-smoke harness-e2e

help:
	@echo "Local benchmark commands:"
	@echo "  make game GAME=ls20                         sandboxed, model-backed game"
	@echo "  make game-engine GAME=ls20 STEPS=50         free engine-only inner loop"
	@echo "  make harness-e2e                            Academy + Rust stack locally"
	@echo ""
	@echo "Use SUBMISSION_REPO for the model-backed game or ENGINE_SUBMISSION_REPO offline."

benchmark-venv:
	@bash services/benchmark-node/local.sh setup

game:
	@GAME="$(GAME)" GAME_SECONDS="$(GAME_SECONDS)" SUBMISSION_REPO="$(SUBMISSION_REPO)" \
		bash services/benchmark-node/local.sh game

game-engine:
	@GAME="$(GAME)" STEPS="$(STEPS)" ENGINE_SUBMISSION_REPO="$(ENGINE_SUBMISSION_REPO)" \
		bash services/benchmark-node/local.sh game-engine

harness-smoke: game

harness-e2e:
	@bash services/benchmark-node/local.sh stack
