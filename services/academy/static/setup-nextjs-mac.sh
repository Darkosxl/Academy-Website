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

# --- corepack: whether this needs sudo depends entirely on where the node above
# came from (nvm = user-owned bin dir, pre-existing system install = often root-owned),
# so decide it once from node's actual bin dir and reuse it for every corepack call ---
corepack_cmd() {
  local bin_dir
  bin_dir="$(dirname "$(command -v node)")"
  if [ -w "$bin_dir" ]; then
    corepack "$@"
  else
    echo "corepack needs to write into $bin_dir, which $(whoami) doesn't own — using sudo."
    sudo corepack "$@"
  fi
}

corepack_cmd enable
corepack_cmd prepare pnpm@latest --activate
corepack_cmd prepare yarn@stable --activate   # ponytail: delete this block if you don't want yarn

# --- Self-check: everything must resolve on PATH right now, in this same shell ---
echo
echo "Checking PATH..."
ok=1
for cmd in "git --version" "node -v" "corepack -v" "pnpm -v" "yarn -v"; do
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
