
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
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Use the custom config file path instead of default')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Verbose output - show detailed information about operations')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Verbose output - show detailed information about operations')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('add-with-exclude', 'add-with-exclude', [CompletionResultType]::ParameterValue, 'Add all files to the `git add` command and exclude the patterns passed as positional arguments')
            [CompletionResult]::new('commit', 'commit', [CompletionResultType]::ParameterValue, 'Directly commit the file with the text in `commit_message.md`')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completions for your shell')
            [CompletionResult]::new('config', 'config', [CompletionResultType]::ParameterValue, 'Manage configuration files (create or edit local or global config)')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Directly generate the `commit_message.md` file')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'Initialize the rona configuration file')
            [CompletionResult]::new('list-status', 'list-status', [CompletionResultType]::ParameterValue, 'List files from git status (for shell completion on the -a)')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Push to a git repository')
            [CompletionResult]::new('set-editor', 'set-editor', [CompletionResultType]::ParameterValue, 'Set the editor to use for editing the commit message')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rona;add-with-exclude' {
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be added without actually adding files')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;commit' {
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Whether to push the commit after committing')
            [CompletionResult]::new('--push', '--push', [CompletionResultType]::ParameterName, 'Whether to push the commit after committing')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Show what would be committed without actually committing')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be committed without actually committing')
            [CompletionResult]::new('-u', '-u', [CompletionResultType]::ParameterName, 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)')
            [CompletionResult]::new('--unsigned', '--unsigned', [CompletionResultType]::ParameterName, 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)')
            [CompletionResult]::new('-y', '-y', [CompletionResultType]::ParameterName, 'Skip confirmation prompt and commit directly')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'Skip confirmation prompt and commit directly')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;completion' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;config' {
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be created without actually creating the config file')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'rona;generate' {
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
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be initialized without creating files')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;list-status' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;push' {
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be pushed without actually pushing')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;set-editor' {
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Show what would be changed without modifying config')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rona;help' {
            [CompletionResult]::new('add-with-exclude', 'add-with-exclude', [CompletionResultType]::ParameterValue, 'Add all files to the `git add` command and exclude the patterns passed as positional arguments')
            [CompletionResult]::new('commit', 'commit', [CompletionResultType]::ParameterValue, 'Directly commit the file with the text in `commit_message.md`')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate shell completions for your shell')
            [CompletionResult]::new('config', 'config', [CompletionResultType]::ParameterValue, 'Manage configuration files (create or edit local or global config)')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Directly generate the `commit_message.md` file')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'Initialize the rona configuration file')
            [CompletionResult]::new('list-status', 'list-status', [CompletionResultType]::ParameterValue, 'List files from git status (for shell completion on the -a)')
            [CompletionResult]::new('push', 'push', [CompletionResultType]::ParameterValue, 'Push to a git repository')
            [CompletionResult]::new('set-editor', 'set-editor', [CompletionResultType]::ParameterValue, 'Set the editor to use for editing the commit message')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
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
        'rona;help;set-editor' {
            break
        }
        'rona;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
