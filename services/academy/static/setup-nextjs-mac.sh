#!/usr/bin/env bash
# Idempotent Next.js dev environment setup for macOS: Xcode CLT, Homebrew, git, nvm, Node LTS, pnpm/yarn.
# Safe to re-run.
set -euo pipefail

have() { command -v "$1" >/dev/null 2>&1; }

# --- Xcode Command Line Tools ---
if ! xcode-select -p >/dev/null 2>&1; then
  echo "Installing Xcode Command Line Tools (a GUI installer will open)..."
  xcode-select --install
  echo "Finish that installer, then run this script again."
  exit 0
fi

# --- Homebrew (and put it on PATH now + in every future shell) ---
BREW_BIN="/opt/homebrew/bin/brew"
[ -x "$BREW_BIN" ] || BREW_BIN="/usr/local/bin/brew"

if [ ! -x "$BREW_BIN" ]; then
  echo "Installing Homebrew..."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  [ -x "/opt/homebrew/bin/brew" ] && BREW_BIN="/opt/homebrew/bin/brew" || BREW_BIN="/usr/local/bin/brew"
fi

eval "$("$BREW_BIN" shellenv)"

for rc in "$HOME/.zprofile" "$HOME/.bash_profile"; do
  [ -f "$rc" ] || touch "$rc"
  grep -q "brew shellenv" "$rc" 2>/dev/null || echo "eval \"\$($BREW_BIN shellenv)\"" >> "$rc"
done

have git || brew install git

# --- nvm (and auto-activate default node in every future shell) ---
export NVM_DIR="$HOME/.nvm"
if [ ! -s "$NVM_DIR/nvm.sh" ]; then
  echo "Installing nvm..."
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
fi
# shellcheck disable=SC1091
. "$NVM_DIR/nvm.sh"

for rc in "$HOME/.zshrc" "$HOME/.bash_profile"; do
  [ -f "$rc" ] || touch "$rc"
  grep -q "NVM_DIR" "$rc" 2>/dev/null || cat >> "$rc" <<'EOF'

export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
nvm use default --silent >/dev/null 2>&1
EOF
done

# --- Node: leave an existing >=18 install alone, otherwise install LTS and make it default ---
if have node && [ "$(node -v | sed 's/^v//' | cut -d. -f1)" -ge 18 ]; then
  echo "Node $(node -v) already installed, leaving it alone."
else
  nvm install --lts
  nvm alias default 'lts/*'
  nvm use default
fi

# --- pnpm / yarn, resolved against the node we settled on above ---
# Three traps on machines that already have tooling installed:
#   1. Node >=25 doesn't bundle corepack, so a bare `corepack` falls through PATH to
#      an unrelated install — often root-owned, from an old Node .pkg -> EACCES.
#   2. `corepack enable` picks its target dir from which('corepack'), a PATH lookup,
#      so invoking it by absolute path is NOT enough; pass --install-directory too.
#   3. `npm i -g corepack` collides (EEXIST) with an existing standalone pnpm/yarn.
# So: use corepack only when it ships with the active node, else install directly.
NODE_BIN_DIR="$(dirname "$(command -v node)")"
export PATH="$NODE_BIN_DIR:$PATH"

# A system node (e.g. /usr/local from a .pkg) has a root-owned bin dir; nvm's doesn't.
node_run() {
  if [ -w "$NODE_BIN_DIR" ]; then
    "$@"
  else
    echo "Writing into $NODE_BIN_DIR needs privileges $(whoami) lacks — using sudo."
    sudo "$@"
  fi
}

if [ -x "$NODE_BIN_DIR/corepack" ]; then
  node_run "$NODE_BIN_DIR/corepack" enable --install-directory "$NODE_BIN_DIR"
  node_run "$NODE_BIN_DIR/corepack" prepare pnpm@latest --activate
  node_run "$NODE_BIN_DIR/corepack" prepare yarn@stable --activate   # ponytail: delete this line if you don't want yarn
else
  echo "No corepack ships with $(node -v) — installing package managers with npm instead."
  [ -x "$NODE_BIN_DIR/pnpm" ] || node_run "$NODE_BIN_DIR/npm" install -g pnpm
  [ -x "$NODE_BIN_DIR/yarn" ] || node_run "$NODE_BIN_DIR/npm" install -g yarn   # ponytail: delete this line if you don't want yarn
fi

# --- Self-check: everything must resolve on PATH right now, in this same shell ---
echo
echo "Checking PATH..."
ok=1
for cmd in "git --version" "node -v" "pnpm -v" "yarn -v"; do
  if out=$(eval "$cmd" 2>&1); then
    echo "  OK   $cmd -> $out"
  else
    echo "  FAIL $cmd -> $out"
    ok=0
  fi
done

if [ "$ok" -eq 1 ]; then
  echo "Done. Everything is on PATH."
else
  echo "Some tools aren't resolving yet in this shell. Open a brand new terminal tab and re-run this script."
  exit 1
fi
