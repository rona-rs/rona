# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_rona_global_optspecs
	string join \n v/verbose config= h/help V/version
end

function __fish_rona_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_rona_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_rona_using_subcommand
	set -l cmd (__fish_rona_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c rona -n "__fish_rona_needs_command" -l config -d 'Use the custom config file path instead of default' -r
complete -c rona -n "__fish_rona_needs_command" -s v -l verbose -d 'Verbose output - show detailed information about operations'
complete -c rona -n "__fish_rona_needs_command" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_needs_command" -s V -l version -d 'Print version'
complete -c rona -n "__fish_rona_needs_command" -f -a "add-with-exclude" -d 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
complete -c rona -n "__fish_rona_needs_command" -f -a "commit" -d 'Directly commit the file with the text in `commit_message.md`'
complete -c rona -n "__fish_rona_needs_command" -f -a "completion" -d 'Generate shell completions for your shell'
complete -c rona -n "__fish_rona_needs_command" -f -a "config" -d 'Manage configuration files (create or edit local or global config)'
complete -c rona -n "__fish_rona_needs_command" -f -a "generate" -d 'Directly generate the `commit_message.md` file'
complete -c rona -n "__fish_rona_needs_command" -f -a "init" -d 'Initialize the rona configuration file'
complete -c rona -n "__fish_rona_needs_command" -f -a "list-status" -d 'List files from git status (for shell completion on the -a)'
complete -c rona -n "__fish_rona_needs_command" -f -a "push" -d 'Push to a git repository'
complete -c rona -n "__fish_rona_needs_command" -f -a "set-editor" -d 'Set the editor to use for editing the commit message'
complete -c rona -n "__fish_rona_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rona -n "__fish_rona_using_subcommand add-with-exclude" -l dry-run -d 'Show what would be added without actually adding files'
complete -c rona -n "__fish_rona_using_subcommand add-with-exclude" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand commit" -s p -l push -d 'Whether to push the commit after committing'
complete -c rona -n "__fish_rona_using_subcommand commit" -s d -l dry-run -d 'Show what would be committed without actually committing'
complete -c rona -n "__fish_rona_using_subcommand commit" -s u -l unsigned -d 'Create unsigned commit (default is to auto-detect GPG availability and sign if possible)'
complete -c rona -n "__fish_rona_using_subcommand commit" -s y -l yes -d 'Skip confirmation prompt and commit directly'
complete -c rona -n "__fish_rona_using_subcommand commit" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand completion" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand config" -l dry-run -d 'Show what would be created without actually creating the config file'
complete -c rona -n "__fish_rona_using_subcommand config" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rona -n "__fish_rona_using_subcommand generate" -l dry-run -d 'Show what would be generated without creating files'
complete -c rona -n "__fish_rona_using_subcommand generate" -s i -l interactive -d 'Interactive mode - input the commit message directly in the terminal'
complete -c rona -n "__fish_rona_using_subcommand generate" -s n -l no-commit-number -d 'No commit number'
complete -c rona -n "__fish_rona_using_subcommand generate" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand init" -l dry-run -d 'Show what would be initialized without creating files'
complete -c rona -n "__fish_rona_using_subcommand init" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand list-status" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand push" -l dry-run -d 'Show what would be pushed without actually pushing'
complete -c rona -n "__fish_rona_using_subcommand push" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand set-editor" -l dry-run -d 'Show what would be changed without modifying config'
complete -c rona -n "__fish_rona_using_subcommand set-editor" -s h -l help -d 'Print help'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "add-with-exclude" -d 'Add all files to the `git add` command and exclude the patterns passed as positional arguments'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "commit" -d 'Directly commit the file with the text in `commit_message.md`'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "completion" -d 'Generate shell completions for your shell'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "config" -d 'Manage configuration files (create or edit local or global config)'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "generate" -d 'Directly generate the `commit_message.md` file'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "init" -d 'Initialize the rona configuration file'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "list-status" -d 'List files from git status (for shell completion on the -a)'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "push" -d 'Push to a git repository'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "set-editor" -d 'Set the editor to use for editing the commit message'
complete -c rona -n "__fish_rona_using_subcommand help; and not __fish_seen_subcommand_from add-with-exclude commit completion config generate init list-status push set-editor help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'

# === CUSTOM RONA COMPLETIONS ===
# Helper function to get git status files
function __rona_status_files
    rona -l
end

# Command-specific completions
# add-with-exclude: Complete with git status files
complete -c rona -n '__fish_seen_subcommand_from add-with-exclude -a' -xa '(__rona_status_files)'
