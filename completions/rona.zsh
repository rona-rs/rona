#compdef rona

autoload -U is-at-least

_rona() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'--config=[Use the custom config file path instead of default]:PATH:_default' \
'-v[Verbose output - show detailed information about operations]' \
'--verbose[Verbose output - show detailed information about operations]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_rona_commands" \
"*::: :->rona" \
&& ret=0
    case $state in
    (rona)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:rona-command-$line[1]:"
        case $line[1] in
            (add-with-exclude)
_arguments "${_arguments_options[@]}" : \
'--dry-run[Show what would be added without actually adding files]' \
'-h[Print help]' \
'--help[Print help]' \
'*::to_exclude -- Patterns of files to exclude (supports glob patterns like `"node_modules/*"`):_files' \
&& ret=0
;;
(commit)
_arguments "${_arguments_options[@]}" : \
'-p[Whether to push the commit after committing]' \
'--push[Whether to push the commit after committing]' \
'-d[Show what would be committed without actually committing]' \
'--dry-run[Show what would be committed without actually committing]' \
'-u[Create unsigned commit (default is to auto-detect GPG availability and sign if possible)]' \
'--unsigned[Create unsigned commit (default is to auto-detect GPG availability and sign if possible)]' \
'-h[Print help]' \
'--help[Print help]' \
'*::args -- Additional arguments to pass to the commit command:_default' \
&& ret=0
;;
(completion)
_arguments "${_arguments_options[@]}" : \
'-h[Print help]' \
'--help[Print help]' \
':shell -- The shell to generate completions for:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(generate)
_arguments "${_arguments_options[@]}" : \
'--dry-run[Show what would be generated without creating files]' \
'-i[Interactive mode - input the commit message directly in the terminal]' \
'--interactive[Interactive mode - input the commit message directly in the terminal]' \
'-n[No commit number]' \
'--no-commit-number[No commit number]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(init)
_arguments "${_arguments_options[@]}" : \
'--dry-run[Show what would be initialized without creating files]' \
'-h[Print help]' \
'--help[Print help]' \
'::editor -- Editor to use for the commit message:_default' \
&& ret=0
;;
(list-status)
_arguments "${_arguments_options[@]}" : \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(push)
_arguments "${_arguments_options[@]}" : \
'--dry-run[Show what would be pushed without actually pushing]' \
'-h[Print help]' \
'--help[Print help]' \
'*::args -- Additional arguments to pass to the push command:_default' \
&& ret=0
;;
(set-editor)
_arguments "${_arguments_options[@]}" : \
'--dry-run[Show what would be changed without modifying config]' \
'-h[Print help]' \
'--help[Print help]' \
':editor -- The editor to use for the commit message:_default' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_rona__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:rona-help-command-$line[1]:"
        case $line[1] in
            (add-with-exclude)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(commit)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(completion)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(generate)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(init)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list-status)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(push)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(set-editor)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_rona_commands] )) ||
_rona_commands() {
    local commands; commands=(
'add-with-exclude:Add all files to the \`git add\` command and exclude the patterns passed as positional arguments' \
'commit:Directly commit the file with the text in \`commit_message.md\`' \
'completion:Generate shell completions for your shell' \
'generate:Directly generate the \`commit_message.md\` file' \
'init:Initialize the rona configuration file' \
'list-status:List files from git status (for shell completion on the -a)' \
'push:Push to a git repository' \
'set-editor:Set the editor to use for editing the commit message' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'rona commands' commands "$@"
}
(( $+functions[_rona__add-with-exclude_commands] )) ||
_rona__add-with-exclude_commands() {
    local commands; commands=()
    _describe -t commands 'rona add-with-exclude commands' commands "$@"
}
(( $+functions[_rona__commit_commands] )) ||
_rona__commit_commands() {
    local commands; commands=()
    _describe -t commands 'rona commit commands' commands "$@"
}
(( $+functions[_rona__completion_commands] )) ||
_rona__completion_commands() {
    local commands; commands=()
    _describe -t commands 'rona completion commands' commands "$@"
}
(( $+functions[_rona__generate_commands] )) ||
_rona__generate_commands() {
    local commands; commands=()
    _describe -t commands 'rona generate commands' commands "$@"
}
(( $+functions[_rona__help_commands] )) ||
_rona__help_commands() {
    local commands; commands=(
'add-with-exclude:Add all files to the \`git add\` command and exclude the patterns passed as positional arguments' \
'commit:Directly commit the file with the text in \`commit_message.md\`' \
'completion:Generate shell completions for your shell' \
'generate:Directly generate the \`commit_message.md\` file' \
'init:Initialize the rona configuration file' \
'list-status:List files from git status (for shell completion on the -a)' \
'push:Push to a git repository' \
'set-editor:Set the editor to use for editing the commit message' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'rona help commands' commands "$@"
}
(( $+functions[_rona__help__add-with-exclude_commands] )) ||
_rona__help__add-with-exclude_commands() {
    local commands; commands=()
    _describe -t commands 'rona help add-with-exclude commands' commands "$@"
}
(( $+functions[_rona__help__commit_commands] )) ||
_rona__help__commit_commands() {
    local commands; commands=()
    _describe -t commands 'rona help commit commands' commands "$@"
}
(( $+functions[_rona__help__completion_commands] )) ||
_rona__help__completion_commands() {
    local commands; commands=()
    _describe -t commands 'rona help completion commands' commands "$@"
}
(( $+functions[_rona__help__generate_commands] )) ||
_rona__help__generate_commands() {
    local commands; commands=()
    _describe -t commands 'rona help generate commands' commands "$@"
}
(( $+functions[_rona__help__help_commands] )) ||
_rona__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'rona help help commands' commands "$@"
}
(( $+functions[_rona__help__init_commands] )) ||
_rona__help__init_commands() {
    local commands; commands=()
    _describe -t commands 'rona help init commands' commands "$@"
}
(( $+functions[_rona__help__list-status_commands] )) ||
_rona__help__list-status_commands() {
    local commands; commands=()
    _describe -t commands 'rona help list-status commands' commands "$@"
}
(( $+functions[_rona__help__push_commands] )) ||
_rona__help__push_commands() {
    local commands; commands=()
    _describe -t commands 'rona help push commands' commands "$@"
}
(( $+functions[_rona__help__set-editor_commands] )) ||
_rona__help__set-editor_commands() {
    local commands; commands=()
    _describe -t commands 'rona help set-editor commands' commands "$@"
}
(( $+functions[_rona__init_commands] )) ||
_rona__init_commands() {
    local commands; commands=()
    _describe -t commands 'rona init commands' commands "$@"
}
(( $+functions[_rona__list-status_commands] )) ||
_rona__list-status_commands() {
    local commands; commands=()
    _describe -t commands 'rona list-status commands' commands "$@"
}
(( $+functions[_rona__push_commands] )) ||
_rona__push_commands() {
    local commands; commands=()
    _describe -t commands 'rona push commands' commands "$@"
}
(( $+functions[_rona__set-editor_commands] )) ||
_rona__set-editor_commands() {
    local commands; commands=()
    _describe -t commands 'rona set-editor commands' commands "$@"
}

if [ "$funcstack[1]" = "_rona" ]; then
    _rona "$@"
else
    compdef _rona rona
fi
