<pre>
██▄████   ▄████▄   ██▄████▄   ▄█████▄ 
██▀      ██▀  ▀██  ██▀   ██   ▀ ▄▄▄██ 
██       ██    ██  ██    ██  ▄██▀▀▀██ 
██       ▀██▄▄██▀  ██    ██  ██▄▄▄███ 
▀▀         ▀▀▀▀    ▀▀    ▀▀   ▀▀▀▀ ▀▀ 
</pre>


> A powerful CLI tool to streamline your Git workflow

> Created and maintained by [Tom Planche](https://github.com/TomPlanche). The GitHub organization exists solely to host the Homebrew tap alongside the main repository.

<p align="center">
  <a href="https://crates.io/crates/rona"><img src="https://shieldcn.dev/crates/v/rona.svg?variant=secondary&variant=secondary&theme=slate" alt="Crates.io Version"></a>
  <a href="https://https://github.com/rona-rs/homebrew-rona"><img alt="Brew/GitHub Release" src="https://shieldcn.dev/github/release/rona-rs/rona.svg?variant=secondary&theme=slate"></a>
  <a href="https://github.com/rona-rs/rona/blob/main/LICENSE"><img src="https://shieldcn.dev/github/rona-rs/rona/license.svg?variant=secondary&theme=slate" alt="License"></a>
  <a href="https://github.com/rona-rs/rona/actions/workflows/rust.yaml"><img src="https://shieldcn.dev/github/rona-rs/rona/ci.svg?variant=secondary&theme=slate" alt="Build Status"></a>
  <br>
  <a href="https://sonarcloud.io/summary/new_code?id=TomPlanche_rona"><img src="https://sonarcloud.io/api/project_badges/measure?project=TomPlanche_rona&metric=alert_status" alt="SonarCloud Status"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=TomPlanche_rona"><img src="https://sonarcloud.io/api/project_badges/measure?project=TomPlanche_rona&metric=sqale_rating" alt="SonarCloud SQALE Rating"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=TomPlanche_rona"><img src="https://sonarcloud.io/api/project_badges/measure?project=TomPlanche_rona&metric=security_rating" alt="SonarCloud Security Rating"></a>
</p>


## Documentation

Full documentation is available in the [GitHub Wiki](https://github.com/rona-rs/rona/wiki):

- [Home](https://github.com/rona-rs/rona/wiki/Home) — project overview and architecture
- [Features](https://github.com/rona-rs/rona/wiki/Features) — in-depth feature reference
- [Configuration](https://github.com/rona-rs/rona/wiki/Configuration) — all config options with examples
- [Usage Guide](https://github.com/rona-rs/rona/wiki/Usage-Guide) — workflows and recipes
- [Command Reference](https://github.com/rona-rs/rona/wiki/Command-Reference) — every flag and subcommand
- [Shell Integration](https://github.com/rona-rs/rona/wiki/Shell-Integration) — completions setup per shell
- [FAQ](https://github.com/rona-rs/rona/wiki/FAQ) — common questions

## Overview

Rona is a command-line interface tool designed to enhance your Git workflow with powerful features and intuitive commands. It simplifies common Git operations and provides additional functionality for managing commits, files, and repository status.

## Features

- Intelligent file staging with pattern exclusion, working correctly from any subdirectory of the repository, including filenames with spaces
- Interactive unstage and discard with checklists (`rona reset`, `rona restore`)
- Structured commit message generation
- Interactive branch creation from configurable name templates (`rona branch`)
- Streamlined push operations
- Branch synchronization with merge/rebase support
- Interactive commit type selection with customizable types
- Config-driven extra prompt fields (scope, ticket, etc.) with optional prefetching, regex validation, and configurable ordering — for both commit messages and branch names
- Multi-shell completion support (Bash, Fish, Zsh, PowerShell)
- Flexible configuration system (global, project-level, and custom file via `-f`/`--config-file`)
- Colored interactive prompts powered by Inquire
- Structured logging via `tracing` with `RUST_LOG` support

## Installation

### Homebrew (macOS/Linux)

```bash
brew install rona-rs/rona/rona
```

Or, if you prefer to tap explicitly:

```bash
brew tap rona-rs/rona
brew install rona
```

### Cargo (Alternative)

```bash
cargo install rona
```

After installation, initialize Rona (optional, to set your preferred editor):

```bash
rona init [editor] # The editor to use for commit messages (default: nano)
```

## Configuration

Rona supports flexible configuration through TOML files:

- **Global config**: `~/.config/rona.toml` - applies to all projects
- **Project config**: `./.rona.toml` - applies only to the current project (overrides global)
- **Custom config**: any TOML file passed via `-f <PATH>` / `--config-file <PATH>` - bypasses the default hierarchy entirely
- **Extended config**: a `.rona.toml` containing only `extends = "path/to/config.toml"` delegates all settings to another file

```bash
# Use a custom config file instead of the default global/project one
rona -f /path/to/my-config.toml -g -i

# Useful for testing different configs or CI environments
rona -f .rona.ci.toml -c -p
```

### Configuration Options

```toml
# Editor for commit messages (any command-line editor)
editor = "nano"  # Examples: "vim", "zed", "code --wait", "emacs"

# Custom commit types (used by both rona -g and rona branch)
commit_types = [
    "feat",    # New features
    "fix",     # Bug fixes
    "docs",    # Documentation changes
    "test",    # Adding or updating tests
    "chore"    # Maintenance tasks
]

# Optional: dedicated types shown only in the rona branch type selector.
# When absent, commit_types is used instead.
# branch_types = ["feat", "fix", "hotfix", "release"]

# When true and branch_types is set, the selector shows branch_types followed
# by any commit_types not already present in it.
# Default: false (only branch_types is shown when branch_types is set).
# merge_branch_and_commit_types = false

# Template for interactive commit message generation
# Built-in variables: {commit_number}, {commit_type}, {branch_name}, {message}, {date}, {time}, {author}, {email}
# Extra field names defined in [[extra_fields]] are also valid template variables.
template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}"

# Optional: control the order of prompts in interactive mode.
# Use "message" to position the built-in message prompt among the extra fields.
# Unlisted extra fields are appended after all listed items.
# Default (empty): extra fields first, then message.
# field_order = ["message", "scope", "ticket"]

# Template for branch name generation (rona branch).
# Built-in variables: {commit_type}, {description}, {date}, {time}, {author}
# Extra field names defined in [[branch_extra_fields]] are also valid.
# The result is sanitized automatically (lowercased, spaces to "-", etc.).
# branch_template = "{commit_type}/{description}"

# Optional: control the order of prompts in rona branch.
# Use "description" to position the built-in description prompt.
# branch_field_order = ["ticket", "description"]

# Extra prompts shown after commit type selection (see "Extra Fields" section below)
# [[extra_fields]]
# name = "scope"
# ...

# Extra prompts for branch name generation (see "Branch Extra Fields" section below)
# [[branch_extra_fields]]
# name = "ticket"
# ...
```

**Note**: When no configuration exists, Rona falls back to: `["chore", "feat", "fix", "test"]`

### Shared Configuration with `extends`

A `.rona.toml` can point to another TOML file using the `extends` key. The referenced file is loaded first, then the current file's values override it. This is useful for sharing a common base config across multiple projects or repositories.

```toml
# .rona.toml — delegate everything to a shared file
extends = "~/configs/rona-base.toml"
```

You can still override individual fields on top of the extended file:

```toml
extends = "~/configs/rona-base.toml"
editor = "vim"  # overrides whatever editor is set in the base
```

The path is resolved relative to the `.rona.toml` file that declares it. Absolute paths are also supported. If the referenced file does not exist, Rona exits with a clear error.

#### Unreferenced extra fields are skipped

When a config extends another, extra fields are merged by `name`: same-name fields are overridden by the child, and fields the child does not redefine are inherited from the base. Rona only prompts for an extra field that the active template actually references (as `{name}` or `{?name}`). If an inherited field is not referenced by your template, it is skipped with a `[NOTE]` line instead of asking you for a value that would be discarded. This applies independently to `rona branch` (checked against `branch_template`) and `rona -g -i` (checked against `commit_template`).

For example, given a base config that defines a `ticket` field and uses it in both templates:

```toml
# ~/configs/rona-base.toml
branch_template = "{branch_type}/{?ticket}{ticket}_{/ticket}{description}"
commit_template = "{commit_type}({scope}): {message} {ticket}"

[[branch_extra_fields]]
name = "ticket"
kind = "text"

[[commit_extra_fields]]
name = "scope"
kind = "select"

[[commit_extra_fields]]
name = "ticket"
kind = "text"
```

And a project config that overrides `branch_template` (dropping `{ticket}`) but leaves `commit_template` inherited:

```toml
# .rona.toml
extends = "~/configs/rona-base.toml"
branch_template = "{branch_type}/{service}/{version}"

[[branch_extra_fields]]
name = "service"
kind = "select"

[[branch_extra_fields]]
name = "version"
kind = "text"
```

Then:

- `rona branch` prompts for `service` and `version` only. The inherited `ticket` field is not in `branch_template`, so it is skipped:

  ```text
  [NOTE] Branch extra field 'ticket' is not referenced in the template; skipping.
  ```

- `rona -g -i` still prompts for both `scope` and `ticket`, because the inherited `commit_template` references `{scope}` and `{ticket}`. Nothing is skipped on the commit side.

### Path-Conditional Configuration with `[[overrides]]`

Where `extends` is declared by the project that wants a shared base, `[[overrides]]` works the other way round: the global config declares that any repository under a given directory should pick up an extra config file, without those repositories needing a `.rona.toml` at all.

```toml
# ~/.config/rona.toml
editor = "nano"

# When run anywhere under ~/Affluences, layer in the referenced config.
[[overrides]]
path = "~/Affluences/**"
config = "~/Affluences/.rona.config"

[[overrides]]
path = "~/oss"
config = "~/configs/rona-oss.toml"
```

- `path` is a glob matched against the directory Rona runs from. A leading `~/` is expanded to your home directory. `~/Affluences/**` matches `~/Affluences` itself and every directory beneath it, and a pattern with no wildcard (like `~/oss`) does the same for that directory and all of its descendants.
- `config` is the config file to layer in. Relative paths resolve against the config file that declares the override, and `~/` is expanded. The referenced file may itself use `extends`.
- Every matching override is applied, in declaration order, so later entries win over earlier ones.
- If the referenced file does not exist, the override is skipped silently. Run `rona config -w` (or `rona config which`) to see exactly which files are being layered in from the current directory; each override row names the `path` pattern that matched.

**On Windows**, write paths with forward slashes, or use TOML *literal* strings (single quotes) so backslashes are not treated as escape sequences. `path = "C:\Users\me\work\**"` is invalid TOML, because `\U` is not a valid escape:

```toml
[[overrides]]
path = 'C:\Users\me\work\**'   # literal string: backslashes are literal
config = 'C:\Users\me\work\shared-rona.toml'

# equivalent, and simpler
[[overrides]]
path = "C:/Users/me/work/**"
config = "C:/Users/me/work/shared-rona.toml"
```

Both separators work, and matching is case-insensitive on Windows.

The resulting precedence, lowest to highest, is: legacy global config, global config, matching `[[overrides]]` targets, the project config's `extends` chain, then the project `.rona.toml` itself.

### Template Configuration

Rona supports customizable templates for interactive commit message generation. You can define how your commit messages are formatted using variables:

**Available Template Variables:**

- `{commit_number}` - The commit number (incremental)
- `{commit_type}` - The selected commit type (feat, fix, etc.)
- `{branch_name}` - The current branch name
- `{message}` - Your input message
- `{date}` - Current date (YYYY-MM-DD)
- `{time}` - Current time (HH:MM:SS)
- `{author}` - Git author name
- `{email}` - Git author email
- `{name}` - Any extra field defined under `[[extra_fields]]` (e.g. `{scope}`, `{ticket}`)

**Conditional Blocks:**

You can use conditional blocks to include or exclude content based on whether a variable has a value. This is useful for handling optional elements like commit numbers.

**Syntax:** `{?variable_name}content{/variable_name}`

The content inside the block will only be included if the variable has a non-empty value.

**Example with `-n` flag:**

```toml
# Template with conditional commit number
template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}"
```

**Results:**

- `rona -g` (with commit number): `[42] (feat on new-feature) Add feature`
- `rona -g -n` (without commit number): `(feat on new-feature) Add feature`

This eliminates empty brackets when using the `-n` flag!

**Template Examples:**

```toml
# Default template with conditional commit number
template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}"

# Simple format without commit number
template = "({commit_type}) {message}"

# Conditional date with static text
template = "{?date}Date: {date} | {/date}{commit_type}: {message}"

# Multiple conditional blocks
template = "{?commit_number}#{commit_number} {/commit_number}{?author}by {author} - {/author}{message}"

# Include date and time conditionally
template = "{?date}[{date} {time}] {/date}{commit_type}: {message}"

# Custom format with optional commit number
template = "{?commit_number}Commit {commit_number}: {/commit_number}{commit_type} on {branch_name} - {message}"
```

**Note**: If no template is specified, Rona uses the default format: `{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}`

### Branch Name Template

`rona branch` uses a dedicated template to generate branch names. After template processing the result is automatically sanitized: lowercased, spaces and unsupported characters replaced with `-`, consecutive `-` and `/` collapsed, and leading/trailing `-` trimmed from each path segment.

**Available Template Variables:**

- `{commit_type}` - The selected commit type (feat, fix, etc.)
- `{description}` - The branch description entered by the user
- `{date}` - Current date (YYYY-MM-DD)
- `{time}` - Current time (HH:MM:SS)
- `{author}` - Git author name (from git config)
- `{name}` - Any extra field defined under `[[branch_extra_fields]]` (e.g. `{ticket}`)

Conditional blocks work exactly as they do for commit templates.

**Configuration:**

```toml
# Default template (used when branch_template is absent)
branch_template = "{commit_type}/{description}"

# With optional ticket reference: feat/PROJ-123/add-login
branch_template = "{commit_type}/{?ticket}{ticket}/{/ticket}{description}"

# Date-prefixed for time-bounded branches: 2024-01-15/feat/add-login
branch_template = "{date}/{commit_type}/{description}"

# Per-developer namespace: johndoe/feat/add-login
branch_template = "{author}/{commit_type}/{description}"
```

**Prompt order** is controlled by `branch_field_order`, using the reserved name `"description"` for the built-in description prompt:

```toml
# Show ticket prompt first, then description (default when branch_extra_fields exist)
branch_field_order = ["ticket", "description"]

# Description first, then any extra fields
branch_field_order = ["description", "ticket"]
```

**Session example** (`rona branch` with the ticket template above):

```
$ Select commit type
> feat

$ Ticket reference (optional)
> PROJ-42

$ Branch description
> add login endpoint
```

Generated branch name: `feat/proj-42/add-login-endpoint`

**Type selector:**

By default the branch type selector shows `commit_types`. Two config keys let you customize this independently:

- `branch_types` - a dedicated list shown only in `rona branch`, replacing `commit_types` in that selector
- `merge_branch_and_commit_types` - when `true` and `branch_types` is set, the selector shows `branch_types` followed by any `commit_types` not already present in it (default: `false`)

```toml
branch_types = ["feat", "fix", "hotfix", "release"]
merge_branch_and_commit_types = true  # adds remaining commit_types after branch_types
```

**Options:**

- `--no-switch` - Create the branch without switching to it
- `--dry-run` - Preview the generated name without creating the branch

### Extra Fields

Extra fields let you declare additional prompts in `.rona.toml` that are shown after the commit type selector and before the message. Each field becomes a template variable using its `name`, so you can embed it in the `template` option with full conditional block support.

This replaces the need for a separate tool when a project requires additional structured inputs such as a component scope or a ticket reference.

**Field options:**

| Key                      | Type                      | Required | Description                                                  |
| ------------------------ | ------------------------- | -------- | ------------------------------------------------------------ |
| `name`                   | string                    | yes      | Variable name used in templates (`{scope}`, `{ticket}`, etc.) |
| `prompt`                 | string                    | no       | Label shown to the user. Defaults to `name`.                 |
| `kind`                   | `"text"` \| `"select"`    | no       | Input style. Default: `"text"`.                              |
| `required`               | bool                      | no       | Whether an empty answer is rejected. Default: `false`.       |
| `validation`             | string                    | no       | Regex the answer must match.                                 |
| `prefetch.source`        | `"command"` \| `"branch"` | no       | Where to fetch candidate values from.                        |
| `prefetch.command`       | string                    | no       | Shell command to run (for `source = "command"`).             |
| `prefetch.extract_regex` | string                    | no       | Regex applied to each output line or the branch name. Priority: named group `value`, then capture group 1, then full match. |
| `prefetch.deduplicate`   | bool                      | no       | Remove duplicate results (for `source = "command"`). Default: `false`. |

**Prompt behaviour by kind and prefetch:**

| `kind`   | Prefetch result               | Behaviour                                                    |
| -------- | ----------------------------- | ------------------------------------------------------------ |
| `select` | non-empty list                | Select from list + `(none)` (if optional) + `Other (enter manually)` |
| `select` | empty                         | Falls back to a free-text prompt                             |
| `text`   | non-empty list from `command` | Same as `select` with non-empty list                         |
| `text`   | 0–1 values from `branch`      | Free-text prompt with the extracted value as the default     |
| `text`   | nothing                       | Plain free-text prompt                                       |

When a field is skipped (optional + user chose `(none)`), the variable is simply absent. Use a conditional block in your template to handle this cleanly: `{?scope}({scope}){/scope}`.

#### Prompt order

By default, extra fields are shown first (in declaration order), then the built-in `message` prompt. Use `field_order` to change this:

```toml
# Show message first, then scope, then ticket
field_order = ["message", "scope", "ticket"]
```

The reserved name `"message"` positions the built-in message prompt. Any extra field not listed in `field_order` is appended after the last listed item. `message` is always included — if you omit it from `field_order`, it is appended at the very end.

**TOML ordering note**: In TOML, every key-value pair after a `[[extra_fields]]` header belongs to that array item — not to the top-level table. Always place `template`, `editor`, and `commit_types` **before** any `[[extra_fields]]` entry.

#### Example: scope and ticket

The example below replicates a conventional-commit workflow where the scope is suggested from recent commits and the ticket number is extracted automatically from the branch name.

**.rona.toml**

```toml
editor = "zed"
commit_types = ["feat", "fix", "docs", "refactor", "test", "chore"]

# Template uses both built-in variables and the extra fields defined below
template = "{?commit_number}[{commit_number}] {/commit_number}{commit_type}{?scope}({scope}){/scope}: {message}{?ticket} [{ticket}]{/ticket}"

# --- scope ---
# Populated by scanning the last 20 commit subjects.
# Extracts text between parentheses, e.g. "feat(api): ..." -> "api"
[[extra_fields]]
name = "scope"
prompt = "Select scope"
kind = "select"
required = false
prefetch.source = "command"
prefetch.command = "git log -20 --pretty=format:%s"
prefetch.extract_regex = "\\w+\\((?P<value>[^)]*)\\):"
prefetch.deduplicate = true

# --- ticket ---
# Default value extracted from the branch name, e.g. "feat/PROJ-123_add-login" -> "PROJ-123"
# User can edit it or leave it empty if not required.
[[extra_fields]]
name = "ticket"
prompt = "Ticket reference"
kind = "text"
required = false
validation = "^[A-Z]+-[0-9]+$"
prefetch.source = "branch"
prefetch.extract_regex = "[A-Z]+-[0-9]+"
```

**Session example** (`rona -g -i` on branch `feat/PROJ-42_add-login`, default order: scope → ticket → message):

```
$ Select commit type
> feat

$ Select scope
> api
  auth
  (none)
  Other (enter manually)

$ Ticket reference (PROJ-42)
> PROJ-42

$ Message
> Add login endpoint
```

To prompt for the message first, add `field_order = ["message", "scope", "ticket"]` above the `[[extra_fields]]` entries.

**Resulting commit message:**

```
feat(api): Add login endpoint [PROJ-42]
```

Or, with commit number enabled:

```
[7] feat(api): Add login endpoint [PROJ-42]
```

If the user skips the scope, the conditional block is omitted:

```
feat: Add login endpoint [PROJ-42]
```

#### Example: static select options (no prefetch)

If you just want a fixed list without any prefetching, omit the `prefetch` block and list `kind = "select"` — but note that without prefetch, an empty candidate list causes the prompt to fall back to a free-text input. For a true fixed list, provide the options via `prefetch.command` using a shell command like `echo`:

```toml
[[extra_fields]]
name = "env"
prompt = "Target environment"
kind = "select"
required = true
prefetch.source = "command"
prefetch.command = "printf 'staging\\nproduction\\n'"
prefetch.extract_regex = "(.+)"
```

For the full configuration reference including all options and edge cases, see the [Configuration wiki page](https://github.com/rona-rs/rona/wiki/Configuration).

### Branch Extra Fields

`[[branch_extra_fields]]` entries work exactly like `[[extra_fields]]` but are shown during `rona branch` instead of `rona -g -i`. They support the same keys (`name`, `prompt`, `kind`, `required`, `validation`, `prefetch.*`) and the values become template variables in `branch_template`.

**Example: ticket reference prepended to the branch name**

```toml
branch_template = "{commit_type}/{?ticket}{ticket}/{/ticket}{description}"
branch_field_order = ["ticket", "description"]

[[branch_extra_fields]]
name = "ticket"
prompt = "Ticket reference (optional)"
kind = "text"
required = false
validation = "^[A-Z]+-[0-9]+$"
prefetch.source = "branch"
prefetch.extract_regex = "[A-Z]+-[0-9]+"
```

Session on branch `feat/PROJ-42_some-work`:

```
$ Select commit type
> feat

$ Ticket reference (optional) (PROJ-42)
> PROJ-42

$ Branch description
> add login endpoint
```

Generated: `feat/proj-42/add-login-endpoint`

If the user skips the ticket (optional field), the conditional block removes it:

Generated: `feat/add-login-endpoint`

**TOML ordering note**: Place `branch_template` and `branch_field_order` **before** any `[[branch_extra_fields]]` entry.

### Working with Configuration

```bash
# Initialize global configuration
rona init vim                    # Creates ~/.config/rona.toml

# Initialize project-specific configuration
cd my-project
rona init zed                    # Creates ./.rona.toml (overrides global)

# Create a local config and exclude it from git tracking
rona config create local --exclude
rona config -c local -e          # Short form

# Create a global config file
rona config create global
rona config -c global            # Short form

# Change editor later
rona set-editor "code --wait"    # Choose global or project scope interactively

# Inspect which config files are active
rona config which                # Show sources for current directory
rona config -w                   # Short form
rona config which --effective    # Also show merged values

# View current configuration
cat .rona.toml                   # Project config
cat ~/.config/rona.toml          # Global config

# Customize commit types for your project
echo 'commit_types = ["feat", "fix", "refactor", "style", "docs"]' >> .rona.toml
```

## Usage Examples

For more complete workflows and recipes, see the [Usage Guide](https://github.com/rona-rs/rona/wiki/Usage-Guide).


### Basic Workflow

1. Initialize Rona with your preferred editor:

```bash
# Initialize with various editors
rona init vim
rona init zed  
rona init "code --wait"  # VS Code
rona init emacs

# Initialize with default editor (nano)
rona init
```

2. Stage files while excluding specific patterns:

```bash
# Exclude Rust files
rona -a "*.rs"

# Exclude multiple file types
rona -a "*.rs" "*.tmp" "*.log"

# Exclude directories
rona -a "target/" "node_modules/"

# Exclude files with specific patterns
rona -a "test_*.rs" "*.test.js"

# Or pick files interactively from a checklist
rona -a -i
```

3. Generate and edit commit message:

```bash
# Generate commit message template (opens editor)
rona -g

# Interactive mode (input directly in terminal)
rona -g -i

# This will:
# 1. Open an interactive commit type selector
# 2. Create/update commit_message.md
# 3. Either open your configured editor (default) or prompt for simple input (-i)
```

4. Commit and push changes:

```bash
# Commit with the prepared message (auto-detects GPG and signs if available)
rona -c

# Create an unsigned commit (explicitly disable signing)
rona -c -u
# or
rona -c --unsigned

# Commit and push in one command
rona -c -p

# Commit with additional Git arguments
rona -c --no-verify

# Unsigned commit with push
rona -c -u -p

# Commit and push with specific branch
rona -c -p origin main
```

### Advanced Usage

#### Working with Multiple Branches

```bash
# Create and switch to a new feature branch
git checkout -b feature/new-feature
rona -a "*.rs"
rona -g
rona -c -p

# Switch back to main and merge
git checkout main
git merge feature/new-feature

# Or use the sync command to update your branch with latest main
git checkout feature/new-feature
rona sync              # Merges main into current branch

# Update branch with rebase instead of merge
rona sync --rebase     # Rebases current branch onto main

# Create new branch and sync with develop
rona sync -b develop -n feature/new-feature

# Preview sync operation
rona sync --dry-run
```

#### Handling Large Changes

```bash
# Stage specific directories
rona -a "src/" "tests/"

# Exclude test files while staging
rona -a "src/" -e "test_*.rs"

# Stage everything except specific patterns
rona -a "*" -e "*.log" "*.tmp"
```

#### Using with CI/CD

```bash
# In your CI pipeline
rona init
rona -a "*"
rona -g
rona -c -p --no-verify
```

#### Shell Integration

```bash
# Fish shell
echo "function rona
    command rona \$argv
end" >> ~/.config/fish/functions/rona.fish

# Bash
echo 'alias rona="command rona"' >> ~/.bashrc
```

### Common Use Cases

1. **Feature Development**:

```bash
# Start new feature
git checkout -b feature/new-feature
rona -a "src/" "tests/"
rona -g  # Select 'feat' type
rona -c -p
```

2. **Bug Fixes**:

```bash
# Fix a bug
git checkout -b fix/bug-description
rona -a "src/"
rona -g  # Select 'fix' type
rona -c -p
```

3. **Code Cleanup**:

```bash
# Clean up code
git checkout -b chore/cleanup
rona -a "src/" -e "*.rs"
rona -g  # Select 'chore' type
rona -c -p
```

4. **Testing**:

```bash
# Add tests
git checkout -b test/add-tests
rona -a "tests/"
rona -g  # Select 'test' type
rona -c -p
```

5. **Quick Commits (Interactive Mode)**:

```bash
# Fast workflow without opening editor
rona -a "src/"
rona -g -i  # Select type and input message directly
rona -c -p
```

## Global Flags

These flags apply to all commands and are placed before the subcommand:

| Flag                    | Short | Description                                                  |
| ----------------------- | ----- | ------------------------------------------------------------ |
| `--config-file <PATH>`  | `-f`  | Load a specific TOML config file, bypassing global and project config |
| `--verbose`             | `-v`  | Enable debug-level log output                                |

```bash
rona -f .rona.toml -g -i
rona --verbose -c -p
rona -f ~/.config/rona-work.toml sync
```

## Command Reference

For the full command reference, see the [Command Reference wiki page](https://github.com/rona-rs/rona/wiki/Command-Reference).


### `branch`

Create a new branch interactively using a configurable branch name template.

```bash
rona branch [--no-switch] [--dry-run]
```

**What it does:**

1. Shows the commit type selector (same list as `rona -g`)
2. Shows prompts for any configured `branch_extra_fields` and the branch description, in the order defined by `branch_field_order`
3. Processes the `branch_template` with the collected values
4. Sanitizes the result into a valid git branch name
5. Creates the branch and switches to it (or just creates it with `--no-switch`)

**Options:**

- `--no-switch` - Create the branch without switching to it (`git branch` instead of `git switch -c`)
- `--dry-run` - Show what branch would be created without making any changes

**Examples:**

```bash
# Interactive branch creation (creates and switches)
rona branch

# Create without switching
rona branch --no-switch

# Preview the generated branch name
rona branch --dry-run
```

**Output example** (with default template `{commit_type}/{description}`):

```
$ Select commit type
> feat

$ Branch description
> add user authentication

Switched to new branch: feat/add-user-authentication
```

### `add-with-exclude` (`-a`)

Add files to Git staging while excluding specified patterns. Paths are always resolved relative to the repository root, so the command works correctly regardless of which subdirectory you run it from. Filenames containing spaces or other special characters are handled correctly.

```bash
rona add-with-exclude <pattern(s)>
# or
rona -a <pattern(s)>
```

**Options:**

- `-i, --interactive` - Pick files to stage from a checklist instead of using exclude patterns
- `--dry-run` - Preview what would be staged without staging anything

**Example:**

```bash
rona -a "*.rs" "*.tmp"  # Exclude Rust and temporary files

# Works from any subdirectory — no path-doubling issues
cd packages/preview/my-pkg/1.0
rona -a  # Correctly stages files relative to the repo root
```

**Interactive mode (`-i`):**

Instead of describing what to leave out with exclude patterns, pick exactly what to stage from a checklist of changed files (similar to `git add -p` or the lazygit file selector). Use the arrow keys to move, space to toggle a file, and enter to confirm. Untracked, modified, type-changed and deleted files are all listed with a short status label.

```bash
rona -a -i  # Open a MultiSelect of changed files and stage the selected ones
```

When `-i` is used, any exclude patterns are ignored.

### `commit` (`-c`)

Commit changes using prepared message. **By default, automatically detects GPG availability and signs commits if possible**.

```bash
rona commit [OPTIONS] [extra args]
# or
rona -c [-p | --push] [-u | --unsigned] [extra args]
```

**Options:**

- `-p, --push` - Push after committing
- `-u, --unsigned` - Create unsigned commit (explicitly disable signing)
- `--dry-run` - Preview what would be committed

**Examples:**

```bash
# Auto-detected signing (default behavior)
rona -c

# Explicitly unsigned commit
rona -c -u

# Commit and push (with auto-detected signing)
rona -c -p

# Explicitly unsigned commit with push
rona -c -u -p
```

### `completion`

Generate shell completion scripts.

```bash
rona completion <shell>
```

**Supported shells:** `bash`, `fish`, `zsh`, `powershell`

**Example:**

```bash
rona completion fish > ~/.config/fish/completions/rona.fish
```

### `config`

Manage configuration files and inspect which ones are active. Groups two subcommands:

#### `config create` (`-c`)

Create a local or global configuration file.

```bash
rona config create <local|global> [--exclude] [--dry-run]
# short form
rona config -c <local|global> [-e] [--dry-run]
```

**Options:**

- `-e, --exclude` - Add `.rona.toml` to `.git/info/exclude` (local scope only)
- `--dry-run` - Preview what would be created without writing any files

**Examples:**

```bash
# Create a local config file
rona config create local
rona config -c local

# Create and exclude .rona.toml from git tracking
rona config create local --exclude
rona config -c local -e

# Create a global config file
rona config create global
rona config -c global

# Preview without writing
rona config create local --dry-run
```

#### `config which` (`-w`)

Show which configuration files would be loaded from the current (or given) directory, in priority order.

```bash
rona config which [PATH] [--effective]
# short form
rona config -w [PATH] [-e | --effective]
```

**Options:**

- `-e, --effective` - Also print the merged configuration values

**Examples:**

```bash
# Show config sources for the current directory
rona config which
rona config -w

# Show config sources from a specific path
rona config which /path/to/project

# Show config sources and their merged values
rona config which --effective
rona config -w -e
```

### `generate` (`-g`)

Generate or update commit message template.

```bash
rona generate [--interactive] [--no-commit-number]
# or
rona -g [-i | --interactive] [-n | --no-commit-number]
```

**Features:**

- Creates `commit_message.md` and `.commitignore`
- Interactive commit type selection
- Automatic file change tracking
- **Interactive mode:** Input commit message directly in terminal (`-i` flag)
- **Editor mode:** Opens in configured editor (default behavior)
- **No commit number:** Omit commit number from message (`-n` flag)

**Options:**

- `-i, --interactive` - Input commit message directly in terminal instead of opening editor
- `-n, --no-commit-number` - Generate commit message without commit number

**Examples:**

```bash
# Standard mode: Opens commit type selector, then editor
rona -g

# Interactive mode: Input message directly in terminal
rona -g -i

# Without commit number (useful with conditional templates)
rona -g -n

# Interactive mode without commit number
rona -g -i -n
```

**Interactive Mode Usage:**
When using the `-i` flag, Rona will:

1. Show the commit type selector (uses configured types or defaults: feat, fix, docs, test, chore)
2. Show prompts for any configured extra fields and the message, in the order defined by `field_order` (defaults to extra fields first, then message)
3. Generate a clean format using your template (or default)
4. Save directly to `commit_message.md` without file details

**No Commit Number Flag:**
The `-n` flag sets `commit_number` to `None`, which works perfectly with conditional templates:

- With conditional template: `{?commit_number}[{commit_number}] {/commit_number}({commit_type}) {message}`
- Result with `-n`: `(feat) Add feature` (no empty brackets!)
- Result without `-n`: `[42] (feat) Add feature`

This is perfect for quick, clean commits without the detailed file listing.

### Prompt UI and Colors

Rona uses the `dialoguer` crate for interactive prompts with a custom color scheme shared across every prompt:

- Prompt prefix: `$` (light red)
- Success prefix: `✓` (light green)
- Error prefix: `✕` (light red)
- Highlighted option prefix: `➠` (light blue)
- Multi-select checkboxes: `[x]` (light green) / `[ ]` (black)
- Prompt label: light cyan + bold
- Hint message: yellow + italic
- Answer text: light magenta + bold

If you prefer different colors, you can fork and adjust the shared theme in `src/theme.rs` (function `prompt_theme`), which every prompt receives via `with_theme(...)`.

Single-choice prompts (commit type, branch type, and other selection fields) use a fuzzy `FuzzySelect`: start typing to filter the list instead of scrolling through it with the arrow keys. File pickers remain multi-select checkboxes.

**Commit Types:**

- Uses commit types from your configuration (`.rona.toml` or `~/.config/rona.toml`)
- Falls back to: `["chore", "feat", "fix", "test"]` when no configuration exists
- Default configuration includes: `["feat", "fix", "docs", "test", "chore"]`

### `init` (`-i`)

Initialize Rona configuration.

```bash
rona init [editor] # Any command-line editor (default: nano)
```

**Examples:**

```bash
rona init vim
rona init zed  
rona init "code --wait"  # VS Code
rona init                # Uses default (nano)
```

### `list-status` (`-l`)

Display repository status (primarily for shell completion).

```bash
rona list-status
# or
rona -l
```

### `push` (`-p`)

Push committed changes to remote repository.

```bash
rona push [extra args]
# or
rona -p [extra args]
```

### `reset`

Unstage files, moving them out of the staging area without losing any changes. This is the inverse of `add` and is a safe, non-destructive operation: your working-tree edits are preserved.

```bash
rona reset [FILES...]
```

**Options:**

- `-i, --interactive` - Pick which staged files to unstage from a checklist
- `--dry-run` - Preview what would be unstaged without changing anything

**Behavior:**

- With explicit `FILES`, only those files are unstaged.
- With no arguments, every staged file is unstaged (like `git reset`).
- With `-i`, a `MultiSelect` of staged files is shown and only the selected ones are unstaged.

**Examples:**

```bash
rona reset                 # Unstage everything currently staged
rona reset src/main.rs     # Unstage a single file
rona reset -i              # Pick staged files to unstage from a checklist
rona reset --dry-run       # Preview which files would be unstaged
```

### `restore`

Discard working-tree changes, restoring files to their staged (or committed) state. This is **destructive**: unstaged edits to the affected files are lost, so a confirmation prompt is shown before anything is discarded (unless `--yes` or `--dry-run` is used). Untracked files are never touched.

```bash
rona restore [FILES...]
```

**Options:**

- `-i, --interactive` - Pick which changed files to discard from a checklist
- `-y, --yes` - Skip the confirmation prompt
- `--dry-run` - Preview what would be restored without discarding anything

**Behavior:**

- With explicit `FILES`, those files are restored after confirmation.
- With `-i`, a `MultiSelect` of changed (tracked) files is shown and only the selected ones are discarded.
- With neither `FILES` nor `-i`, the command is a no-op and prints a hint, since discarding every change at once is rarely intended.

**Examples:**

```bash
rona restore src/main.rs   # Discard changes to a single file (with confirmation)
rona restore -i            # Pick changed files to discard from a checklist
rona restore -y src/main.rs  # Discard without the confirmation prompt
rona restore --dry-run src/main.rs  # Preview which files would be restored
```

### `set-editor` (`-s`)

Set the default editor for commit messages.

```bash
rona set-editor <editor> # Any command-line editor (vim, zed, "code --wait", etc.)
```

**Examples:**

```bash
rona set-editor vim
rona set-editor zed
rona set-editor "code --wait"  # VS Code
rona set-editor emacs
rona set-editor nano
```

### `sync`

Sync your current branch with another branch by pulling latest changes and merging or rebasing.

```bash
rona sync [OPTIONS]
```

**Options:**

- `-b, --branch <BRANCH>` - Branch to sync from (default: main)
- `-r, --rebase` - Use rebase instead of merge
- `-n, --new-branch <NAME>` - Create a new branch before syncing
- `--dry-run` - Preview what would be done

**Workflow:**

1. Optionally creates a new branch (if `-n` specified)
2. Switches to the source branch
3. Pulls latest changes from remote
4. Switches back to your target branch
5. Merges or rebases the source branch into your target branch

**Examples:**

```bash
# Basic usage: sync current branch with main
rona sync

# Sync with a different branch
rona sync --branch develop
rona sync -b staging

# Use rebase instead of merge
rona sync --rebase
rona sync -r

# Create new branch and sync with main
rona sync --new-branch feature/my-feature
rona sync -n bugfix/issue-123

# Create new branch and sync from develop using rebase
rona sync -b develop -r -n feature/new-feature

# Preview what would happen without making changes
rona sync --dry-run

# Combine all options
rona sync -b develop -r -n feature/test --dry-run
```

**Common Use Cases:**

```bash
# Keep feature branch up-to-date with main
git checkout feature/my-feature
rona sync

# Start new feature from latest main
rona sync -n feature/new-feature

# Update branch with staging before deploying
rona sync -b staging

# Rebase feature branch onto latest main for clean history
rona sync --rebase
```

### `help` (`-h`)

Display help information.

```bash
rona help
# or
rona -h
```

## Shell Completion

For per-shell setup instructions, see the [Shell Integration wiki page](https://github.com/rona-rs/rona/wiki/Shell-Integration).


Rona supports auto-completion for multiple shells using [`clap_complete`](https://docs.rs/clap_complete/latest/clap_complete/index.html).

### Generate Completions

Generate completion files for your shell:

```bash
# Generate completions for specific shell
rona completion fish    # Fish shell
rona completion bash    # Bash
rona completion zsh     # Zsh  
rona completion powershell  # PowerShell

# Save to file
rona completion fish > ~/.config/fish/completions/rona.fish
```

### Installation by Shell

**Fish Shell:**

```fish
# Copy to Fish completions directory
rona completion fish > ~/.config/fish/completions/rona.fish
```

**Bash:**

```bash
# Add to your .bashrc
rona completion bash >> ~/.bashrc
source ~/.bashrc
```

**Zsh:**

```bash
# Add to your .zshrc or save to a completions directory
rona completion zsh >> ~/.zshrc
```

**PowerShell:**

```powershell
# Add to your PowerShell profile
rona completion powershell | Out-File -Append $PROFILE
```

### Features

The completions include:

- All command and flag completions
- Git status file completion for `add-with-exclude` command (Fish only)
- Context-aware suggestions

## Debugging and Logging

Rona uses the [`tracing`](https://crates.io/crates/tracing) ecosystem for structured, filterable log output. All internal debug information (git command decisions, signing checks, file staging counts, etc.) is emitted as `debug`-level trace events rather than unconditional `println!` calls.

### Enabling Debug Output

**Via the `--verbose` flag:**

The `-v` / `--verbose` flag sets the minimum log level to `debug`, which reveals all internal operations:

```bash
rona -v -c           # Show debug traces while committing
rona -v -c -p        # Show debug traces for commit + push
rona -v sync         # Show debug traces for branch sync
```

Example output with `--verbose`:

```
2024-01-15T14:30:00.123Z DEBUG Committing files... unsigned=false dry_run=false
2024-01-15T14:30:00.250Z DEBUG commit successful!
2024-01-15T14:30:00.251Z DEBUG Running git push args=[] dry_run=false
2024-01-15T14:30:01.100Z DEBUG push successful!
```

**Via the `RUST_LOG` environment variable:**

`RUST_LOG` takes precedence over `--verbose` and provides fine-grained module-level filtering using the standard [`EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) syntax.

```bash
# Show all debug output (equivalent to --verbose)
RUST_LOG=debug rona -c

# Show debug output only for the remote module (push/pull)
RUST_LOG=rona::git::remote=debug rona -c -p

# Show debug output only for staging
RUST_LOG=rona::git::staging=debug rona -a "*.rs"

# Show debug output for commit operations
RUST_LOG=rona::git::commit=debug rona -c

# Show debug output for branch operations
RUST_LOG=rona::git::branch=debug rona sync

# Combine multiple filters
RUST_LOG=rona::git::commit=debug,rona::git::remote=debug rona -c -p

# Show trace-level output (most verbose, includes span entry/exit)
RUST_LOG=trace rona -c
```

### Log Levels

| Level   | When emitted                                              |
| ------- | --------------------------------------------------------- |
| `warn`  | Always (default). GPG warnings, missing config, etc.      |
| `debug` | With `--verbose` or `RUST_LOG=debug`. Internal decisions. |
| `trace` | Only with `RUST_LOG=trace`. Span entry and exit events.   |

### Available Modules for Filtering

| Module               | Content                                    |
| -------------------- | ------------------------------------------ |
| `rona::git::branch`  | Switch, create branch, pull, merge, rebase |
| `rona::git::commit`  | Commit creation, GPG signing detection     |
| `rona::git::remote`  | Push operations                            |
| `rona::git::staging` | File staging with pattern exclusion        |
| `rona::git`          | Cross-module git output (handle_output)    |

### How It Works

Rona initializes a `tracing-subscriber` once at startup in `cli::run()`, immediately after parsing CLI arguments. The subscriber respects `RUST_LOG` first; if that variable is absent, it falls back to `"debug"` when `--verbose` is set and `"warn"` otherwise.

Functions that perform meaningful git work are annotated with `#[tracing::instrument]`, so enabling `trace`-level output also records span entry and exit with the relevant parameters automatically.

## Architecture

### Git Operations

All git operations in Rona delegate to the `git` CLI binary via `std::process::Command`. This means:

- All git hooks (`pre-commit`, `commit-msg`, `post-commit`, `pre-push`, etc.) are triggered naturally on every relevant operation.
- Tools like [hooksmith](https://github.com/rona-rs/hooksmith) work out of the box with `rona -c`.
- GPG signing is handled by git's own configuration (`commit.gpgsign`, `user.signingkey`). Rona passes `--no-gpg-sign` when `--unsigned` is requested and warns when no signing key is configured.

**Operations and their corresponding git commands:**

| Rona operation         | git command                               |
| ---------------------- | ----------------------------------------- |
| Repository detection   | `git rev-parse --git-dir`                 |
| Repo root path         | `git rev-parse --show-toplevel`           |
| Current branch         | `git symbolic-ref --short HEAD`           |
| File status            | `git status --porcelain=v1`               |
| Stage files            | `git add -A`                              |
| Unstage excluded files | `git rm --cached -- <files>`              |
| Commit                 | `git commit -F commit_message.md`         |
| Amend                  | `git commit --amend -F commit_message.md` |
| Commit count           | `git rev-list --count HEAD`               |
| Push                   | `git push`                                |
| Pull                   | `git pull`                                |
| Merge                  | `git merge <branch>`                      |
| Rebase                 | `git rebase <branch>`                     |
| Switch branch          | `git switch <branch>`                     |
| Create branch          | `git switch -c <branch>`                  |
| Create branch (no switch) | `git branch <branch>`                  |

## Development

### Requirements

- Rust 2024 edition or later
- Git 2.23 or later (`git switch` was introduced in 2.23)

### Building from Source

```bash
git clone https://github.com/rona-rs/rona.git
cd rona
cargo build --release
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Support

For bugs, questions, and discussions please use the [GitHub Issues](https://github.com/rona-rs/rona/issues).
