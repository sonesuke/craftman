#!/bin/bash
set -e

# User-space tools are installed under $HOME/.local/bin and $HOME/.cargo/bin.
# Prepend both to PATH so installs and presence checks are consistent across
# re-runs: this script runs non-interactively via `docker exec -u 1000`, where
# the login-shell PATH may not include them yet.
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

# Install the Rust toolchain (rustup). Rust is no longer baked into the image;
# the default profile provides cargo, rustc, rustfmt, and clippy — everything
# `mise run pre-commit` needs. craftman uses rustls, so no system OpenSSL is
# required to build it, but pkg-config/libssl-dev ship in the image for any
# crate that links native libs.
if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing Rust toolchain via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile default
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
else
  echo "Rust toolchain already installed: $(rustc --version)"
fi

# cargo-binstall: fast prebuilt-binary installer. Previously baked into the
# image; install it per-user so the `cargo binstall` calls below work.
if ! command -v cargo-binstall >/dev/null 2>&1; then
  echo "Installing cargo-binstall..."
  curl -L --proto '=https' --tlsv1.2 -sSf \
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
    | bash
fi

# Configure git using GitHub noreply email and credential helper
# (gh is provided by the image; auth comes from the mounted gh config volume).
if command -v gh >/dev/null 2>&1 && gh auth status &>/dev/null; then
  gh auth setup-git
  GH_USER=$(gh api user --jq .login)
  GH_ID=$(gh api user --jq .id)
  git config --global user.name "$GH_USER"
  git config --global user.email "${GH_ID}+${GH_USER}@users.noreply.github.com"
  echo "Git configured as $GH_USER (noreply email)"
else
  echo "Warning: GitHub CLI not authenticated, skipping git config"
fi

# Install cargo tools
echo "Installing cargo tools..."
cargo binstall -y cargo-audit cargo-llvm-cov

# Install Claude CLI
if ! command -v claude >/dev/null 2>&1; then
  echo "Installing Claude CLI..."
  curl -fsSL https://claude.ai/install.sh | bash
else
  echo "Claude CLI already installed: $(claude --version)"
fi

# Configure Claude
if [ -n "$Z_AI_API_KEY" ]; then
  mkdir -p "$HOME/.claude"
  cat > "$HOME/.claude/settings.json" <<EOF
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "$Z_AI_API_KEY",
    "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
    "API_TIMEOUT_MS": "3000000",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW": "1000000",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "glm-4.7",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "glm-5.2[1m]",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "glm-5.2[1m]"
  }
}
EOF
fi

# Configure zsh
AUTOSUGGESTIONS=$(find / -path "*/zsh-autosuggestions/zsh-autosuggestions.zsh" 2>/dev/null | head -1)
SYNTAX_HIGHLIGHTING=$(find / -path "*/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" 2>/dev/null | head -1)

cat > "$HOME/.zshrc" <<OUTER
export PATH="\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH"
alias claude="claude --allow-dangerously-skip-permissions"
eval "\$(mise activate zsh)"

# Completions (needed by worktrunk's shell integration, which appends a wt()
# function later in this file)
autoload -Uz compinit && compinit

# Zsh plugins
${AUTOSUGGESTIONS:+source ${AUTOSUGGESTIONS}}
${SYNTAX_HIGHLIGHTING:+source ${SYNTAX_HIGHLIGHTING}}

# Prompt
setopt PROMPT_SUBST
parse_git_branch() {
  local branch
  branch=\$(git symbolic-ref --short HEAD 2>/dev/null) || return
  echo " (\$branch)"
}
PROMPT='%F{blue}%~%f%F{yellow}\$(parse_git_branch)%f
%F{green}❯%f '
OUTER

# Install mise
if ! command -v mise >/dev/null 2>&1; then
  echo "Installing mise..."
  curl -fsSL https://mise.run | bash
else
  echo "mise already installed: $(mise --version)"
fi

cd /workspaces/craftman
mise trust
mise install
mise generate git-pre-commit -w

# Install worktrunk (git worktree management for parallel AI agent workflows)
# - Shell integration runs AFTER the .zshrc is generated above, otherwise the
#   generated .zshrc would overwrite the wt() function it appends.
# - `binstall -y` and `--yes` skip interactive confirmation prompts (this runs
#   non-interactively via docker exec); `zsh` scopes shell install to our shell.
echo "Installing worktrunk..."
cargo binstall -y worktrunk
wt --yes config shell install zsh

# Configure gh auth for git
if command -v gh >/dev/null 2>&1; then
  echo "Configuring gh auth for git..."
  gh auth setup-git
fi

echo "Setup completed."
