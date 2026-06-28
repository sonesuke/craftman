#!/bin/bash
set -e

# Install GitHub CLI (gh) from the official binary release.
# gh is intentionally NOT managed by Nix (see flake.nix) so it can be upgraded
# independently of nixpkgs' release cadence; the AFK workflow needs 2.94.0+ for
# Issues 2.0 (native sub-issues and relationships). This script runs as user
# 1000 (no root, no apt), so we install user-space into ~/.local/bin.
GH_VERSION="2.95.0"   # pinned to the latest stable at implementation time
GH_MINIMUM="2.94.0"   # minimum required; skip reinstall when gh is already >= this
# ~/.local/bin must be on PATH for this script and for future shells. The .zshrc
# generated below already prepends it; this export covers the current process.
export PATH="$HOME/.local/bin:$PATH"

gh_satisfied() {
  command -v gh >/dev/null 2>&1 || return 1
  local line current
  line="$(gh --version 2>/dev/null | head -1)"   # "gh version X.Y.Z (date)"
  current="${line#* version }"                   # drop "gh version " prefix
  current="${current%% *}"                        # drop trailing " (date)"
  [ -n "$current" ] || return 1
  # current >= GH_MINIMUM iff GH_MINIMUM is the lowest of the two under sort -V
  [ "$(printf '%s\n' "$GH_MINIMUM" "$current" | sort -V | head -1)" = "$GH_MINIMUM" ]
}

if gh_satisfied; then
  echo "gh already installed: $(gh --version | head -1)"
else
  case "$(uname -m)" in
    x86_64)  GH_ARCH="amd64" ;;
    aarch64) GH_ARCH="arm64" ;;
    *) echo "Unsupported architecture for gh: $(uname -m)" >&2; exit 1 ;;
  esac
  echo "Installing gh ${GH_VERSION} (linux_${GH_ARCH})..."
  GH_TARBALL="gh_${GH_VERSION}_linux_${GH_ARCH}.tar.gz"
  curl -fsSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/${GH_TARBALL}" \
    -o "/tmp/${GH_TARBALL}"
  mkdir -p "$HOME/.local/bin"
  tar -xzf "/tmp/${GH_TARBALL}" -C /tmp
  install -m 0755 "/tmp/gh_${GH_VERSION}_linux_${GH_ARCH}/bin/gh" "$HOME/.local/bin/gh"
  rm -rf "/tmp/${GH_TARBALL}" "/tmp/gh_${GH_VERSION}_linux_${GH_ARCH}"
  echo "gh installed: $(gh --version | head -1)"
fi

# Configure git using GitHub noreply email and credential helper
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

# Install cargo tools (Rust is pre-installed in the image)
echo "Installing cargo tools..."
cargo binstall -y cargo-audit cargo-llvm-cov

# Install Claude CLI
if ! command -v claude >/dev/null 2>&1; then
  echo "Installing Claude CLI..."
  curl -fsSL https://claude.ai/install.sh | bash
  export PATH="$HOME/.local/bin:$PATH"
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
  export PATH="$HOME/.local/bin:$PATH"
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
export PATH="$HOME/.cargo/bin:$PATH"
wt --yes config shell install zsh
# Install the Worktrunk plugin for Claude Code (Claude <-> wt integration).
# Runs after shell integration so `wt` is on PATH.
wt --yes config plugins claude install

# Configure gh auth for git
if command -v gh >/dev/null 2>&1; then
  echo "Configuring gh auth for git..."
  gh auth setup-git
fi

echo "Setup completed."
