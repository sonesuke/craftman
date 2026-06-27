# syntax=docker/dockerfile:1
#
# craftman dev container.
#
# Standard Ubuntu base image + setup.sh. No Nix. System tools and gh are
# installed here (as root, at build time) so the image is self-contained and
# non-interactive; per-user runtime tools (Rust, Claude CLI, mise, worktrunk)
# are installed by scripts/setup.sh, which runs as user 1000.
FROM ubuntu:24.04

# Non-interactive apt + locale. C.UTF-8 is always available in glibc, so no
# locales package or generation step is needed.
ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    HOME=/home/user

# Basic tools (user story 13) + zsh with autosuggestions, syntax-highlighting,
# and completions (user story 6) + build essentials so Rust native deps link.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      gnupg \
      zsh \
      zsh-autosuggestions \
      zsh-syntax-highlighting \
      zsh-completions \
      git \
      ripgrep \
      jq \
      vim \
      build-essential \
      pkg-config \
      libssl-dev \
      unzip \
      tar \
      gzip \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI 2.94.0+ (Issues 2.0: native sub-issues and relationships) from the
# official apt repository. The "stable" channel always ships the latest release,
# which is >= 2.94.0.
RUN mkdir -p -m 755 /etc/apt/keyrings \
    && out=$(mktemp) \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o "$out" \
    && cat "$out" | tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null \
    && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update \
    && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# Non-root user matching the previous Nix image: uid/gid 1000, home /home/user,
# zsh login shell.
RUN groupadd -g 1000 user \
    && useradd -m -u 1000 -g 1000 -s /bin/zsh user

# /workspaces must be owned by 1000:1000 so worktrunk can create sibling worktrees
# at /workspaces/craftman.<branch> without "Permission denied". Preserved from the
# Nix image's fakeRootCommands (see issue #17 agent brief).
RUN mkdir -p /workspaces && chown -R 1000:1000 /workspaces

USER 1000:1000
WORKDIR /workspaces/craftman
CMD ["/bin/zsh"]
