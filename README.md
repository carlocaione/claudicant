# claudicant

A TUI tool for reviewing GitHub pull requests with Claude AI assistance.

Fetch a PR, browse commits and diffs with syntax highlighting, let Claude review the code, curate the comments (accept, reject, edit), then submit the review back to GitHub — all from your terminal.

## Prerequisites

- [Rust](https://rustup.rs/) (for building)
- [GitHub CLI](https://cli.github.com/) (`gh`) — authenticated with `gh auth login`
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (`claude`) — installed and working
- `$EDITOR` set to your preferred editor (falls back to `vi`)

## Install

```bash
git clone https://github.com/carlocaione/claudicant.git
cd claudicant
cargo install --path .
```

## Usage

Run from inside a cloned GitHub repository:

```bash
claudicant 42          # Review PR #42
claudicant '#42'       # Also works with #
claudicant 42 --theme ocean-dark --model sonnet --effort high
claudicant 42 --log-file /tmp/claudicant.log
claudicant 42 -r /path/to/repo
```

### CLI options

| Flag | Description |
|------|-------------|
| `-r, --repo <PATH>` | Path to git repo (default: current directory) |
| `--theme <NAME>` | Color theme: `ocean-dark`, `solarized-dark`, `light` |
| `--model <MODEL>` | Claude model: `opus`, `sonnet`, `haiku` |
| `--effort <LEVEL>` | Effort level: `low`, `medium`, `high`, `max` |
| `--log-file <PATH>` | Log Claude responses and GitHub API calls |

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `?` | Help |
| `s` | Settings |
| `p` | PR description |
| `r` | Run Claude review |
| `A` / `X` | Accept / reject all pending comments (current commit) |
| `S` | Submit review to GitHub |
| `Tab` | Switch panel |
| `q` / `Esc` | Quit |

### Commit list

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate commits |
| `g` / `G` | First / last |
| `Enter` / `l` | Open in diff panel |

### Diff panel

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate lines |
| `Ctrl-D` / `Ctrl-U` | Half page down / up |
| `g` / `G` | First / last line |
| `n` / `N` | Next / previous comment |
| `Enter` | Open comment or add new one |
| `h` / `Esc` | Back to commit list |

### Viewing a comment

| Key | Action |
|-----|--------|
| `a` | Accept |
| `x` | Reject |
| `e` / `Enter` | Edit in `$EDITOR` |
| `Esc` | Close |

### Review prompt dialog

| Key | Action |
|-----|--------|
| `a` / `Enter` | Accept and send to Claude |
| `e` | Edit in `$EDITOR` |
| `/` | Select a custom prompt |
| `Esc` | Cancel |

### Submit dialog

| Key | Action |
|-----|--------|
| `a` / `Enter` | Submit |
| `e` | Edit summary in `$EDITOR` |
| `t` | Cycle review type (Comment / Approve / Request Changes) |
| `Esc` | Cancel |

## Configuration

Config files are loaded in order, each overriding the previous:

1. `~/.config/claudicant/config.toml` (global)
2. `.claudicant/config.toml` (project-local)
3. CLI flags

### Example config

```toml
theme = "solarized-dark"
model = "sonnet"
effort = "high"
default_prompt = "basic"
commit_panel_width = 30
```

| Setting | Values | Default |
|---------|--------|---------|
| `theme` | `ocean-dark`, `solarized-dark`, `light` | `ocean-dark` |
| `model` | `opus`, `sonnet`, `haiku` | Claude's default |
| `effort` | `low`, `medium`, `high`, `max` | Claude's default |
| `default_prompt` | Name of a `.md` file in prompts dir | none |
| `commit_panel_width` | 10–80 (percentage) | 30 |

## Custom prompts

Create `.md` files to customize what Claude focuses on during review:

- `~/.config/claudicant/prompts/` — global prompts
- `.claudicant/prompts/` — project-local prompts (shadow global by name)

Example prompt (`.claudicant/prompts/security.md`):

```
Focus on security issues only: injection vulnerabilities, unsafe
deserialization, improper input validation, authentication and
authorization flaws, and sensitive data exposure.
```

The `default_prompt` config setting auto-appends the named prompt to every review. Press `/` in the review prompt dialog to pick a different one.

## How it works

1. Fetches PR metadata and commits from GitHub API
2. Generates full-file diffs locally using `libgit2`
3. Displays commits and diffs in a two-panel TUI with syntax highlighting
4. Invokes `claude -p` with a structured JSON schema for the review
5. Distributes Claude's comments to the correct commits and lines
6. User triages each comment: accept, reject, or edit
7. Submits accepted comments to GitHub as a PR review

## License

MIT
