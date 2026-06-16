
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'rona' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'rona'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'rona' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Verbose output - show detailed information about operations')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Verbose output - show detailed information about operations')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('branch', 'branch', [CompletionResultType]::ParameterValue, 'Create a new branch interactively using a branch name template')
            [CompletionResult]::new('add-with-exclude', 'add-with-exclude', [CompletionResultType]::ParameterValue, 'Add all files to the `git add` command and exclude the patterns passed as positional arguments')
            [CompletionResult]::new('commit', 'commit', [CompletionResultType]::ParameterValue, 'Directly commit the file with the text in `commit_message.md`')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completions for your shell')
            [CompletionResult]::new('config', 'config', [CompletionResultType]::ParameterValue, 'Manage configuration files (create or inspect)')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Directly generate the `commit_message.md` file')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'Initialize the rona configuration file')
            [CompletionResult]::new('list-status', 'list-status', [CompletionResultType]::ParameterValue, 'List files from git status (for shell completion on the -a)')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Push to a git repository')
            [CompletionResult]::new('reset', 'reset', [CompletionResultType]::ParameterValue, 'Unstage files, moving them out of the staging area without losing changes')
            [CompletionResult]::new('restore', 'restore', [CompletionResultType]::ParameterValue, 'Discard working-tree changes, restoring files to their staged or committed state')
            [CompletionResult]::new('set-editor', 'set-editor', [CompletionResultType]::ParameterValue, 'Set the editor to use for editing the commit message')
            [CompletionResult]::new('sync', 'sync', [CompletionResultType]::ParameterValue, 'Sync current branch with main (or another branch) by pulling and merging/rebasing')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rona;branch' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be created without actually creating the branch')
            [CompletionResult]::new('--no-switch', '--no-switch', [CompletionResultType]::ParameterName, 'Create the branch without switching to it')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;add-with-exclude' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Interactively pick which changed files to stage (`MultiSelect` of git status)')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Interactively pick which changed files to stage (`MultiSelect` of git status)')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be added without actually adding files')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;commit' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Whether to push the commit after committing')
            [CompletionResult]::new('--push', '--push', [CompletionResultType]::ParameterName, 'Whether to push the commit after committing')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Show what would be committed without actually committing')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be committed without actually committing')
            [CompletionResult]::new('-u', '-u', [CompletionResultType]::ParameterName, 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)')
            [CompletionResult]::new('--unsigned', '--unsigned', [CompletionResultType]::ParameterName, 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)')
            [CompletionResult]::new('-y', '-y', [CompletionResultType]::ParameterName, 'Skip confirmation prompt and commit directly')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'Skip confirmation prompt and commit directly')
            [CompletionResult]::new('--copy', '--copy', [CompletionResultType]::ParameterName, 'Copy commit message to clipboard instead of committing')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;completion' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;config' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create or manage a local or global configuration file')
            [CompletionResult]::new('which', 'which', [CompletionResultType]::ParameterValue, 'Show which configuration files would be used from a directory')
            [CompletionResult]::new('find', 'find', [CompletionResultType]::ParameterValue, 'Show which configuration files would be used from a directory')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rona;config;create' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-e', '-e', [CompletionResultType]::ParameterName, 'Add .rona.toml to .git/info/exclude (only applies to local scope)')
            [CompletionResult]::new('--exclude', '--exclude', [CompletionResultType]::ParameterName, 'Add .rona.toml to .git/info/exclude (only applies to local scope)')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be created without actually creating the config file')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'rona;config;which' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-e', '-e', [CompletionResultType]::ParameterName, 'Show the effective (merged) configuration values')
            [CompletionResult]::new('--effective', '--effective', [CompletionResultType]::ParameterName, 'Show the effective (merged) configuration values')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;config;find' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-e', '-e', [CompletionResultType]::ParameterName, 'Show the effective (merged) configuration values')
            [CompletionResult]::new('--effective', '--effective', [CompletionResultType]::ParameterName, 'Show the effective (merged) configuration values')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;config;help' {
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create or manage a local or global configuration file')
            [CompletionResult]::new('which', 'which', [CompletionResultType]::ParameterValue, 'Show which configuration files would be used from a directory')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rona;config;help;create' {
            break
        }
        'rona;config;help;which' {
            break
        }
        'rona;config;help;help' {
            break
        }
        'rona;generate' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be generated without creating files')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Interactive mode - input the commit message directly in the terminal')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Interactive mode - input the commit message directly in the terminal')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'No commit number')
            [CompletionResult]::new('--no-commit-number', '--no-commit-number', [CompletionResultType]::ParameterName, 'No commit number')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;init' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be initialized without creating files')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;list-status' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;push' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be pushed without actually pushing')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;reset' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Interactively pick which staged files to unstage (`MultiSelect` of staged files)')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Interactively pick which staged files to unstage (`MultiSelect` of staged files)')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be unstaged without actually unstaging files')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;restore' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Interactively pick which modified files to discard (`MultiSelect` of changed files)')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Interactively pick which modified files to discard (`MultiSelect` of changed files)')
            [CompletionResult]::new('-y', '-y', [CompletionResultType]::ParameterName, 'Skip the confirmation prompt before discarding changes')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'Skip the confirmation prompt before discarding changes')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be restored without actually discarding changes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;set-editor' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be changed without modifying config')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;sync' {
            [CompletionResult]::new('-b', '-b', [CompletionResultType]::ParameterName, 'Branch to sync from (default: main)')
            [CompletionResult]::new('--branch', '--branch', [CompletionResultType]::ParameterName, 'Branch to sync from (default: main)')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Create a new branch before syncing')
            [CompletionResult]::new('--new-branch', '--new-branch', [CompletionResultType]::ParameterName, 'Create a new branch before syncing')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('--config-file', '--config-file', [CompletionResultType]::ParameterName, 'Config file to use instead of the default global/project hierarchy')
            [CompletionResult]::new('-r', '-r', [CompletionResultType]::ParameterName, 'Use rebase instead of merge')
            [CompletionResult]::new('--rebase', '--rebase', [CompletionResultType]::ParameterName, 'Use rebase instead of merge')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be done without actually doing it')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;help' {
            [CompletionResult]::new('branch', 'branch', [CompletionResultType]::ParameterValue, 'Create a new branch interactively using a branch name template')
            [CompletionResult]::new('add-with-exclude', 'add-with-exclude', [CompletionResultType]::ParameterValue, 'Add all files to the `git add` command and exclude the patterns passed as positional arguments')
            [CompletionResult]::new('commit', 'commit', [CompletionResultType]::ParameterValue, 'Directly commit the file with the text in `commit_message.md`')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completions for your shell')
            [CompletionResult]::new('config', 'config', [CompletionResultType]::ParameterValue, 'Manage configuration files (create or inspect)')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Directly generate the `commit_message.md` file')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'Initialize the rona configuration file')
            [CompletionResult]::new('list-status', 'list-status', [CompletionResultType]::ParameterValue, 'List files from git status (for shell completion on the -a)')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Push to a git repository')
            [CompletionResult]::new('reset', 'reset', [CompletionResultType]::ParameterValue, 'Unstage files, moving them out of the staging area without losing changes')
            [CompletionResult]::new('restore', 'restore', [CompletionResultType]::ParameterValue, 'Discard working-tree changes, restoring files to their staged or committed state')
            [CompletionResult]::new('set-editor', 'set-editor', [CompletionResultType]::ParameterValue, 'Set the editor to use for editing the commit message')
            [CompletionResult]::new('sync', 'sync', [CompletionResultType]::ParameterValue, 'Sync current branch with main (or another branch) by pulling and merging/rebasing')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rona;help;branch' {
            break
        }
        'rona;help;add-with-exclude' {
            break
        }
        'rona;help;commit' {
            break
        }
        'rona;help;completion' {
            break
        }
        'rona;help;config' {
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create or manage a local or global configuration file')
            [CompletionResult]::new('which', 'which', [CompletionResultType]::ParameterValue, 'Show which configuration files would be used from a directory')
            break
        }
        'rona;help;config;create' {
            break
        }
        'rona;help;config;which' {
            break
        }
        'rona;help;generate' {
            break
        }
        'rona;help;init' {
            break
        }
        'rona;help;list-status' {
            break
        }
        'rona;help;push' {
            break
        }
        'rona;help;reset' {
            break
        }
        'rona;help;restore' {
            break
        }
        'rona;help;set-editor' {
            break
        }
        'rona;help;sync' {
            break
        }
        'rona;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
