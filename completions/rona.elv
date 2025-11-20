
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
            cand --config 'Use the custom config file path instead of default'
            cand -v 'Verbose output - show detailed information about operations'
            cand --verbose 'Verbose output - show detailed information about operations'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand add-with-exclude 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
            cand commit 'Directly commit the file with the text in `commit_message.md`'
            cand completion 'Generate shell completions for your shell'
            cand generate 'Directly generate the `commit_message.md` file'
            cand init 'Initialize the rona configuration file'
            cand list-status 'List files from git status (for shell completion on the -a)'
            cand push 'Push to a git repository'
            cand set-editor 'Set the editor to use for editing the commit message'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;add-with-exclude'= {
            cand --dry-run 'Show what would be added without actually adding files'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;commit'= {
            cand -p 'Whether to push the commit after committing'
            cand --push 'Whether to push the commit after committing'
            cand -d 'Show what would be committed without actually committing'
            cand --dry-run 'Show what would be committed without actually committing'
            cand -u 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)'
            cand --unsigned 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)'
            cand -y 'Skip confirmation prompt and commit directly'
            cand --yes 'Skip confirmation prompt and commit directly'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;completion'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;generate'= {
            cand --dry-run 'Show what would be generated without creating files'
            cand -i 'Interactive mode - input the commit message directly in the terminal'
            cand --interactive 'Interactive mode - input the commit message directly in the terminal'
            cand -n 'No commit number'
            cand --no-commit-number 'No commit number'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;init'= {
            cand --dry-run 'Show what would be initialized without creating files'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;list-status'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;push'= {
            cand --dry-run 'Show what would be pushed without actually pushing'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;set-editor'= {
            cand --dry-run 'Show what would be changed without modifying config'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'rona;help'= {
            cand add-with-exclude 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
            cand commit 'Directly commit the file with the text in `commit_message.md`'
            cand completion 'Generate shell completions for your shell'
            cand generate 'Directly generate the `commit_message.md` file'
            cand init 'Initialize the rona configuration file'
            cand list-status 'List files from git status (for shell completion on the -a)'
            cand push 'Push to a git repository'
            cand set-editor 'Set the editor to use for editing the commit message'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'rona;help;add-with-exclude'= {
        }
        &'rona;help;commit'= {
        }
        &'rona;help;completion'= {
        }
        &'rona;help;generate'= {
        }
        &'rona;help;init'= {
        }
        &'rona;help;list-status'= {
        }
        &'rona;help;push'= {
        }
        &'rona;help;set-editor'= {
        }
        &'rona;help;help'= {
        }
    ]
    $completions[$command]
}
