//! Tests for the command line grammar.
//!
//! Every test here goes through `Cli::try_parse_from`, so it checks exactly what a user typing
//! that command line gets: the command selected, the flags recognised (short and long), the
//! aliases accepted, and the defaults applied. Nothing here runs a command.

use clap::Parser;

use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

// === ADD COMMAND TESTS ===

#[test]
fn test_add_basic() -> TestResult {
    let args = vec!["rona", "-a"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude {
        to_exclude: exclude,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(exclude.is_empty());
    assert!(!interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_add_single_pattern() -> TestResult {
    let args = vec!["rona", "-a", "*.txt"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude {
        to_exclude: exclude,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(exclude, vec!["*.txt"]);
    assert!(!interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_add_multiple_patterns() -> TestResult {
    let args = vec!["rona", "-a", "*.txt", "*.log", "target/*"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude {
        to_exclude: exclude,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(exclude, vec!["*.txt", "*.log", "target/*"]);
    assert!(!interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_add_with_long_name() -> TestResult {
    let args = vec!["rona", "add-with-exclude", "*.txt"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude {
        to_exclude: exclude,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(exclude, vec!["*.txt"]);
    assert!(!interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_add_interactive() -> TestResult {
    let args = vec!["rona", "-a", "-i"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude {
        to_exclude: exclude,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(exclude.is_empty());
    assert!(interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_add_interactive_long_flag() -> TestResult {
    let args = vec!["rona", "-a", "--interactive"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::AddWithExclude { interactive, .. } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(interactive);
    Ok(())
}

// === RESET COMMAND TESTS ===

#[test]
fn test_reset_basic() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "reset"])?;

    let CliCommand::Reset {
        files,
        interactive,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(files.is_empty());
    assert!(!interactive);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_reset_with_files() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "reset", "src/main.rs", "README.md"])?;

    let CliCommand::Reset { files, .. } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(files, vec!["src/main.rs", "README.md"]);
    Ok(())
}

#[test]
fn test_reset_interactive() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "reset", "-i"])?;

    let CliCommand::Reset { interactive, .. } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(interactive);
    Ok(())
}

#[test]
fn test_reset_dry_run() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "reset", "--dry-run"])?;

    let CliCommand::Reset { dry_run, .. } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(dry_run);
    Ok(())
}

// === RESTORE COMMAND TESTS ===

#[test]
fn test_restore_basic() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "restore"])?;

    let CliCommand::Restore {
        files,
        interactive,
        yes,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(files.is_empty());
    assert!(!interactive);
    assert!(!yes);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_restore_with_files() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "restore", "src/main.rs"])?;

    let CliCommand::Restore { files, .. } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(files, vec!["src/main.rs"]);
    Ok(())
}

#[test]
fn test_restore_interactive_and_yes() -> TestResult {
    let cli = Cli::try_parse_from(["rona", "restore", "-i", "-y"])?;

    let CliCommand::Restore {
        interactive, yes, ..
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(interactive);
    assert!(yes);
    Ok(())
}

// === COMMIT COMMAND TESTS ===

#[test]
fn test_commit_basic() -> TestResult {
    let args = vec!["rona", "-c"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_push_flag() -> TestResult {
    let args = vec!["rona", "-c", "--push"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert!(args.is_empty());
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_message() -> TestResult {
    let args = vec!["rona", "-c", "Regular commit message"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert_eq!(args, vec!["Regular commit message"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_git_flag() -> TestResult {
    let args = vec!["rona", "-c", "--amend"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert_eq!(args, vec!["--amend"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_multiple_git_flags() -> TestResult {
    let args = vec!["rona", "-c", "--amend", "--no-edit"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert_eq!(args, vec!["--amend", "--no-edit"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_push_and_git_flags() -> TestResult {
    let args = vec!["rona", "-c", "--push", "--amend", "--no-edit"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert_eq!(args, vec!["--amend", "--no-edit"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_message_and_push() -> TestResult {
    let args = vec!["rona", "-c", "--push", "Commit message"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert_eq!(args, vec!["Commit message"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

// === PUSH COMMAND TESTS ===

#[test]
fn test_push_basic() -> TestResult {
    let args = vec!["rona", "-p"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Push { args, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(args.is_empty());
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_push_with_force() -> TestResult {
    let args = vec!["rona", "-p", "--force"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Push { args, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(args, vec!["--force"]);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_push_with_multiple_args() -> TestResult {
    let args = vec!["rona", "-p", "--force", "--set-upstream", "origin", "main"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Push { args, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(args, vec!["--force", "--set-upstream", "origin", "main"]);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_push_with_remote_and_branch() -> TestResult {
    let args = vec!["rona", "-p", "origin", "feature/branch"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Push { args, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(args, vec!["origin", "feature/branch"]);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_push_with_upstream_tracking() -> TestResult {
    let args = vec!["rona", "-p", "-u", "origin", "main"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Push { args, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(args, vec!["-u", "origin", "main"]);
    assert!(!dry_run);
    Ok(())
}

// === GENERATE COMMAND TESTS ===

#[test]
fn test_generate_command() -> TestResult {
    let args = vec!["rona", "-g"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(!interactive);
    assert!(!no_commit_number);
    Ok(())
}

#[test]
fn test_generate_interactive_command() -> TestResult {
    let args = vec!["rona", "-g", "-i"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(interactive);
    assert!(!no_commit_number);
    Ok(())
}

#[test]
fn test_generate_interactive_long_form() -> TestResult {
    let args = vec!["rona", "-g", "--interactive"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(interactive);
    assert!(!no_commit_number);
    Ok(())
}

#[test]
fn test_generate_no_commit_number() -> TestResult {
    let args = vec!["rona", "-g", "-n"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(!interactive);
    assert!(no_commit_number);
    Ok(())
}

#[test]
fn test_generate_no_commit_number_long_form() -> TestResult {
    let args = vec!["rona", "-g", "--no-commit-number"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(!interactive);
    assert!(no_commit_number);
    Ok(())
}

#[test]
fn test_generate_interactive_no_commit_number() -> TestResult {
    let args = vec!["rona", "-g", "-i", "-n"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Generate {
        dry_run,
        interactive,
        no_commit_number,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!dry_run);
    assert!(interactive);
    assert!(no_commit_number);
    Ok(())
}

// === LIST STATUS COMMAND TESTS ===

#[test]
fn test_list_status_command() -> TestResult {
    let args = vec!["rona", "-l"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::ListStatus = cli.command else {
        return Err("Wrong command parsed".into());
    };
    Ok(())
}

// === INITIALIZE COMMAND TESTS ===

#[test]
fn test_init_default_editor() -> TestResult {
    let args = vec!["rona", "-i"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Initialize { editor, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(editor, "nano");
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_init_custom_editor() -> TestResult {
    let args = vec!["rona", "-i", "zed"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Initialize { editor, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(editor, "zed");
    assert!(!dry_run);
    Ok(())
}

// === SET EDITOR COMMAND TESTS ===

#[test]
fn test_set_editor() -> TestResult {
    let args = vec!["rona", "-s", "vim"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Set { editor, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(editor, "vim");
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_set_editor_with_spaces() -> TestResult {
    let args = vec!["rona", "-s", "\"Visual Studio Code\""];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Set { editor, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(editor, "\"Visual Studio Code\"");
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_set_editor_with_path() -> TestResult {
    let args = vec!["rona", "-s", "/usr/bin/vim"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Set { editor, dry_run } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(editor, "/usr/bin/vim");
    assert!(!dry_run);
    Ok(())
}

// === VERBOSE FLAG TESTS ===

#[test]
fn test_verbose_with_commit() -> TestResult {
    let args = vec!["rona", "-v", "-c"];
    let cli = Cli::try_parse_from(args)?;
    assert!(cli.verbose);
    Ok(())
}

#[test]
fn test_verbose_with_push() -> TestResult {
    let args = vec!["rona", "-v", "-p"];
    let cli = Cli::try_parse_from(args)?;
    assert!(cli.verbose);
    Ok(())
}

#[test]
fn test_verbose_long_form() -> TestResult {
    let args = vec!["rona", "--verbose", "-c"];
    let cli = Cli::try_parse_from(args)?;
    assert!(cli.verbose);
    Ok(())
}

// === EDGE CASES AND ERROR TESTS ===

#[test]
fn test_commit_flag_order_sensitivity() -> TestResult {
    let args = vec!["rona", "-c", "--amend", "--push"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push); // --push should be treated as git arg
    assert_eq!(args, vec!["--amend", "--push"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_with_similar_looking_args() -> TestResult {
    let args = vec!["rona", "-c", "--push-to-upstream"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert_eq!(args, vec!["--push-to-upstream"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_invalid_command() {
    let args = vec!["rona", "--invalid"];
    assert!(Cli::try_parse_from(args).is_err());
}

#[test]
fn test_missing_required_value() {
    let args = vec!["rona", "-s"]; // missing editor value
    assert!(Cli::try_parse_from(args).is_err());
}

#[test]
fn test_complex_command_combination() -> TestResult {
    let args = vec!["rona", "-v", "-c", "--push", "--amend", "--no-edit"];
    let cli = Cli::try_parse_from(args)?;

    assert!(cli.verbose);
    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert_eq!(args, vec!["--amend", "--no-edit"]);
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_unsigned_short_flag() -> TestResult {
    let args = vec!["rona", "-c", "-u"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(!dry_run);
    assert!(unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_unsigned_long_flag() -> TestResult {
    let args = vec!["rona", "-c", "--unsigned"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(!dry_run);
    assert!(unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_unsigned_with_push_and_args() -> TestResult {
    let args = vec!["rona", "-c", "-u", "--push", "--amend"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert_eq!(args, vec!["--amend"]);
    assert!(!dry_run);
    assert!(unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_dry_run_short_flag() -> TestResult {
    let args = vec!["rona", "-c", "-d"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_dry_run_long_flag() -> TestResult {
    let args = vec!["rona", "-c", "--dry-run"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

#[test]
fn test_commit_dry_run_with_push() -> TestResult {
    let args = vec!["rona", "-c", "-d", "--push"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(push);
    assert!(args.is_empty());
    assert!(dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(!copy);
    Ok(())
}

// === COPY FLAG TESTS ===

#[test]
fn test_commit_with_copy_flag() -> TestResult {
    let args = vec!["rona", "-c", "--copy"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(!dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(copy);
    Ok(())
}

#[test]
fn test_commit_copy_flag_with_other_flags() -> TestResult {
    let args = vec!["rona", "-c", "--copy", "--dry-run"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Commit {
        args,
        push,
        dry_run,
        unsigned,
        yes,
        copy,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert!(!push);
    assert!(args.is_empty());
    assert!(dry_run);
    assert!(!unsigned);
    assert!(!yes);
    assert!(copy);
    Ok(())
}

// === CONFIG COMMAND TESTS ===

fn unwrap_config_create(
    cli: Cli,
) -> std::result::Result<(ConfigScope, bool, bool), Box<dyn std::error::Error>> {
    let CliCommand::Config { subcommand } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    let ConfigSubcommand::Create {
        scope,
        exclude,
        dry_run,
    } = subcommand
    else {
        return Err("Wrong subcommand parsed".into());
    };
    Ok((scope, exclude, dry_run))
}

#[test]
fn test_config_local() -> TestResult {
    let args = vec!["rona", "config", "create", "local"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Local));
    assert!(!exclude);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_config_local_short() -> TestResult {
    let args = vec!["rona", "config", "-c", "local"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Local));
    assert!(!exclude);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_config_global() -> TestResult {
    let args = vec!["rona", "config", "create", "global"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Global));
    assert!(!exclude);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_config_local_dry_run() -> TestResult {
    let args = vec!["rona", "config", "create", "local", "--dry-run"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Local));
    assert!(!exclude);
    assert!(dry_run);
    Ok(())
}

#[test]
fn test_config_global_dry_run() -> TestResult {
    let args = vec!["rona", "config", "create", "global", "--dry-run"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Global));
    assert!(!exclude);
    assert!(dry_run);
    Ok(())
}

#[test]
fn test_config_local_exclude() -> TestResult {
    let args = vec!["rona", "config", "create", "local", "--exclude"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Local));
    assert!(exclude);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_config_local_exclude_short() -> TestResult {
    let args = vec!["rona", "config", "-c", "local", "-e"];
    let cli = Cli::try_parse_from(args)?;
    let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
    assert!(matches!(scope, ConfigScope::Local));
    assert!(exclude);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_config_which() -> TestResult {
    let args = vec!["rona", "config", "which"];
    let cli = Cli::try_parse_from(args)?;
    let CliCommand::Config { subcommand } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    let ConfigSubcommand::Which {
        path,
        show_effective,
    } = subcommand
    else {
        return Err("Wrong subcommand parsed".into());
    };
    assert!(path.is_none());
    assert!(!show_effective);
    Ok(())
}

#[test]
fn test_config_which_short() -> TestResult {
    let args = vec!["rona", "config", "-w"];
    let cli = Cli::try_parse_from(args)?;
    let CliCommand::Config { subcommand } = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(matches!(subcommand, ConfigSubcommand::Which { .. }));
    Ok(())
}

#[test]
fn test_config_missing_subcommand() {
    let args = vec!["rona", "config"];
    assert!(Cli::try_parse_from(args).is_err());
}

#[test]
fn test_config_invalid_scope() {
    let args = vec!["rona", "config", "create", "invalid"];
    assert!(Cli::try_parse_from(args).is_err());
}
// === SYNC COMMAND TESTS ===

#[test]
fn test_sync_basic() -> TestResult {
    let args = vec!["rona", "sync"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(!rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_branch() -> TestResult {
    let args = vec!["rona", "sync", "--branch", "develop"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "develop");
    assert!(!rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_branch_short_flag() -> TestResult {
    let args = vec!["rona", "sync", "-b", "staging"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "staging");
    assert!(!rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_rebase() -> TestResult {
    let args = vec!["rona", "sync", "--rebase"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_rebase_short_flag() -> TestResult {
    let args = vec!["rona", "sync", "-r"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_new_branch() -> TestResult {
    let args = vec!["rona", "sync", "--new-branch", "feature/new-feature"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(!rebase);
    assert_eq!(new_branch, Some("feature/new-feature".to_string()));
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_new_branch_short_flag() -> TestResult {
    let args = vec!["rona", "sync", "-n", "bugfix/issue-123"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(!rebase);
    assert_eq!(new_branch, Some("bugfix/issue-123".to_string()));
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_no_stash() -> TestResult {
    let args = vec!["rona", "sync", "--no-stash"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(!rebase);
    assert!(new_branch.is_none());
    assert!(no_stash);
    assert!(!dry_run);
    Ok(())
}

#[test]
fn test_sync_with_dry_run() -> TestResult {
    let args = vec!["rona", "sync", "--dry-run"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "main");
    assert!(!rebase);
    assert!(new_branch.is_none());
    assert!(!no_stash);
    assert!(dry_run);
    Ok(())
}

#[test]
fn test_sync_all_options() -> TestResult {
    let args = vec![
        "rona",
        "sync",
        "--branch",
        "develop",
        "--rebase",
        "--new-branch",
        "feature/test",
        "--dry-run",
    ];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "develop");
    assert!(rebase);
    assert_eq!(new_branch, Some("feature/test".to_string()));
    assert!(!no_stash);
    assert!(dry_run);
    Ok(())
}

#[test]
fn test_sync_short_flags_combination() -> TestResult {
    let args = vec![
        "rona",
        "sync",
        "-b",
        "staging",
        "-r",
        "-n",
        "hotfix/critical",
    ];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Sync {
        source_branch,
        rebase,
        new_branch,
        no_stash,
        dry_run,
    } = cli.command
    else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(source_branch, "staging");
    assert!(rebase);
    assert_eq!(new_branch, Some("hotfix/critical".to_string()));
    assert!(!no_stash);
    assert!(!dry_run);
    Ok(())
}

// === PR COMMAND TESTS ===

#[test]
fn test_pr_defaults() -> TestResult {
    let args = vec!["rona", "pr"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(pr.target.is_none());
    assert!(pr.title.is_none());
    assert!(pr.body_file.is_none());
    assert!(pr.backend.is_none());
    assert!(pr.remote.is_none());
    assert!(!pr.draft);
    assert!(!pr.web);
    assert!(!pr.no_edit);
    assert!(!pr.no_push);
    assert!(!pr.yes);
    assert!(!pr.dry_run);
    assert!(pr.labels.is_empty());
    assert!(pr.reviewers.is_empty());
    assert!(pr.assignees.is_empty());
    Ok(())
}

#[test]
fn test_pr_mr_alias() -> TestResult {
    let cli = Cli::try_parse_from(vec!["rona", "mr"])?;

    assert!(matches!(cli.command, CliCommand::Pr(_)));
    Ok(())
}

#[test]
fn test_pr_pull_request_alias() -> TestResult {
    let cli = Cli::try_parse_from(vec!["rona", "pull-request"])?;

    assert!(matches!(cli.command, CliCommand::Pr(_)));
    Ok(())
}

#[test]
fn test_pr_target_and_title() -> TestResult {
    let args = vec![
        "rona",
        "pr",
        "--target",
        "develop",
        "--title",
        "Add the pr command",
    ];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(pr.target, Some("develop".to_string()));
    assert_eq!(pr.title, Some("Add the pr command".to_string()));
    Ok(())
}

#[test]
fn test_pr_short_flags() -> TestResult {
    let args = vec!["rona", "pr", "-t", "main", "-T", "Title", "-d", "-y", "-w"];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(pr.target, Some("main".to_string()));
    assert_eq!(pr.title, Some("Title".to_string()));
    assert!(pr.draft);
    assert!(pr.yes);
    assert!(pr.web);
    Ok(())
}

#[test]
fn test_pr_repeated_labels_and_reviewers() -> TestResult {
    let args = vec![
        "rona", "pr", "-l", "bug", "-l", "urgent", "-r", "alice", "-A", "bob",
    ];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(pr.labels, vec!["bug".to_string(), "urgent".to_string()]);
    assert_eq!(pr.reviewers, vec!["alice".to_string()]);
    assert_eq!(pr.assignees, vec!["bob".to_string()]);
    Ok(())
}

#[test]
fn test_pr_backend_accepts_known_names() -> TestResult {
    for backend in ["auto", "gh", "glab", "push-options", "browser"] {
        let cli = Cli::try_parse_from(vec!["rona", "pr", "--backend", backend])?;
        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(pr.backend, Some(backend.to_string()));
    }
    Ok(())
}

#[test]
fn test_pr_backend_rejects_unknown_names() {
    // A typo must fail at parse time rather than silently opening nothing.
    assert!(Cli::try_parse_from(vec!["rona", "pr", "--backend", "carrier-pigeon"]).is_err());
}

#[test]
fn test_pr_body_file_and_no_edit() -> TestResult {
    let args = vec![
        "rona",
        "pr",
        "--body-file",
        "notes.md",
        "--no-edit",
        "--no-push",
    ];
    let cli = Cli::try_parse_from(args)?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert_eq!(pr.body_file, Some("notes.md".to_string()));
    assert!(pr.no_edit);
    assert!(pr.no_push);
    Ok(())
}

#[test]
fn test_pr_dry_run() -> TestResult {
    let cli = Cli::try_parse_from(vec!["rona", "pr", "--dry-run"])?;

    let CliCommand::Pr(pr) = cli.command else {
        return Err("Wrong command parsed".into());
    };
    assert!(pr.dry_run);
    Ok(())
}
