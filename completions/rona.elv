
use builtin;
use str;

set edit:completion:arg-completer[rona] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'rona'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'rona'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -v 'Verbose output - show detailed information about operations'
            cand --verbose 'Verbose output - show detailed information about operations'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand branch 'Create a new branch interactively using a branch name template'
            cand add-with-exclude 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
            cand commit 'Directly commit the file with the text in `commit_message.md`'
            cand completion 'Generate shell completions for your shell'
            cand config 'Manage configuration files (create or inspect)'
            cand generate 'Directly generate the `commit_message.md` file'
            cand init 'Initialize the rona configuration file'
            cand list-status 'List files from git status (for shell completion on the -a)'
            cand push 'Push to a git repository'
            cand reset 'Unstage files, moving them out of the staging area without losing changes'
            cand restore 'Discard working-tree changes, restoring files to their staged or committed state'
            cand set-editor 'Set the editor to use for editing the commit message'
            cand sync 'Sync current branch with main (or another branch) by pulling and merging/rebasing'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;branch'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand --dry-run 'Show what would be created without actually creating the branch'
            cand --no-switch 'Create the branch without switching to it'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;add-with-exclude'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -i 'Interactively pick which changed files to stage (`MultiSelect` of git status)'
            cand --interactive 'Interactively pick which changed files to stage (`MultiSelect` of git status)'
            cand --dry-run 'Show what would be added without actually adding files'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;commit'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -p 'Whether to push the commit after committing'
            cand --push 'Whether to push the commit after committing'
            cand -d 'Show what would be committed without actually committing'
            cand --dry-run 'Show what would be committed without actually committing'
            cand -u 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)'
            cand --unsigned 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)'
            cand -y 'Skip confirmation prompt and commit directly'
            cand --yes 'Skip confirmation prompt and commit directly'
            cand --copy 'Copy commit message to clipboard instead of committing'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;completion'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;config'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -h 'Print help'
            cand --help 'Print help'
            cand create 'Create or manage a local or global configuration file'
            cand which 'Show which configuration files would be used from a directory'
            cand find 'Show which configuration files would be used from a directory'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;config;create'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -e 'Add .rona.toml to .git/info/exclude (only applies to local scope)'
            cand --exclude 'Add .rona.toml to .git/info/exclude (only applies to local scope)'
            cand --dry-run 'Show what would be created without actually creating the config file'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'rona;config;which'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -e 'Show the effective (merged) configuration values'
            cand --effective 'Show the effective (merged) configuration values'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;config;find'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -e 'Show the effective (merged) configuration values'
            cand --effective 'Show the effective (merged) configuration values'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;config;help'= {
            cand create 'Create or manage a local or global configuration file'
            cand which 'Show which configuration files would be used from a directory'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;config;help;create'= {
        }
        &'rona;config;help;which'= {
        }
        &'rona;config;help;help'= {
        }
        &'rona;generate'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand --dry-run 'Show what would be generated without creating files'
            cand -i 'Interactive mode - input the commit message directly in the terminal'
            cand --interactive 'Interactive mode - input the commit message directly in the terminal'
            cand -n 'No commit number'
            cand --no-commit-number 'No commit number'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;init'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand --dry-run 'Show what would be initialized without creating files'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;list-status'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;push'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand --dry-run 'Show what would be pushed without actually pushing'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;reset'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -i 'Interactively pick which staged files to unstage (`MultiSelect` of staged files)'
            cand --interactive 'Interactively pick which staged files to unstage (`MultiSelect` of staged files)'
            cand --dry-run 'Show what would be unstaged without actually unstaging files'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;restore'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -i 'Interactively pick which modified files to discard (`MultiSelect` of changed files)'
            cand --interactive 'Interactively pick which modified files to discard (`MultiSelect` of changed files)'
            cand -y 'Skip the confirmation prompt before discarding changes'
            cand --yes 'Skip the confirmation prompt before discarding changes'
            cand --dry-run 'Show what would be restored without actually discarding changes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;set-editor'= {
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand --dry-run 'Show what would be changed without modifying config'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;sync'= {
            cand -b 'Branch to sync from (default: main)'
            cand --branch 'Branch to sync from (default: main)'
            cand -n 'Create a new branch before syncing'
            cand --new-branch 'Create a new branch before syncing'
            cand -f 'Config file to use instead of the default global/project hierarchy'
            cand --config-file 'Config file to use instead of the default global/project hierarchy'
            cand -r 'Use rebase instead of merge'
            cand --rebase 'Use rebase instead of merge'
            cand --dry-run 'Show what would be done without actually doing it'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;help'= {
            cand branch 'Create a new branch interactively using a branch name template'
            cand add-with-exclude 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
            cand commit 'Directly commit the file with the text in `commit_message.md`'
            cand completion 'Generate shell completions for your shell'
            cand config 'Manage configuration files (create or inspect)'
            cand generate 'Directly generate the `commit_message.md` file'
            cand init 'Initialize the rona configuration file'
            cand list-status 'List files from git status (for shell completion on the -a)'
            cand push 'Push to a git repository'
            cand reset 'Unstage files, moving them out of the staging area without losing changes'
            cand restore 'Discard working-tree changes, restoring files to their staged or committed state'
            cand set-editor 'Set the editor to use for editing the commit message'
            cand sync 'Sync current branch with main (or another branch) by pulling and merging/rebasing'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;help;branch'= {
        }
        &'rona;help;add-with-exclude'= {
        }
        &'rona;help;commit'= {
        }
        &'rona;help;completion'= {
        }
        &'rona;help;config'= {
            cand create 'Create or manage a local or global configuration file'
            cand which 'Show which configuration files would be used from a directory'
        }
        &'rona;help;config;create'= {
        }
        &'rona;help;config;which'= {
        }
        &'rona;help;generate'= {
        }
        &'rona;help;init'= {
        }
        &'rona;help;list-status'= {
        }
        &'rona;help;push'= {
        }
        &'rona;help;reset'= {
        }
        &'rona;help;restore'= {
        }
        &'rona;help;set-editor'= {
        }
        &'rona;help;sync'= {
        }
        &'rona;help;help'= {
        }
    ]
    $completions[$command]
}
