pub const COMMAND_NAMES: &[&str] = &[
    "clear", "cp", "help", "ls", "new", "open", "rm", "run", "save",
];

pub enum TerminalCommand {
    Ls,
    New,
    Save(Option<String>),
    Open(String),
    Rm(String),
    Cp(String, String),
    Run,
    Clear,
    Help,
    Unknown(String),
}

/// Extract the next argument, supporting double-quoted strings.
/// Returns Ok((arg, remaining)) or Err for unbalanced quotes. None if empty.
fn next_arg(s: &str) -> Result<Option<(&str, &str)>, ()> {
    let s = s.trim_start();
    if s.is_empty() {
        return Ok(None);
    }
    if let Some(inner) = s.strip_prefix('"') {
        match inner.find('"') {
            Some(end) => Ok(Some((&inner[..end], inner[end + 1..].trim_start()))),
            None => Err(()),
        }
    } else {
        match s.find(' ') {
            Some(i) => Ok(Some((&s[..i], s[i + 1..].trim_start()))),
            None => Ok(Some((s, ""))),
        }
    }
}

const UNBALANCED_QUOTE: &str = "unbalanced quotes";

pub fn parse_command(input: &str) -> TerminalCommand {
    let trimmed = input.trim();
    let (cmd, rest) = match trimmed.find(' ') {
        Some(i) => (&trimmed[..i], trimmed[i + 1..].trim_start()),
        None => (trimmed, ""),
    };
    let arg = match next_arg(rest) {
        Ok(a) => a,
        Err(()) => return TerminalCommand::Unknown(UNBALANCED_QUOTE.to_string()),
    };
    match cmd {
        "ls" => TerminalCommand::Ls,
        "new" => TerminalCommand::New,
        "save" => TerminalCommand::Save(arg.map(|(s, _)| s.to_string())),
        "open" => match arg {
            Some((name, _)) => TerminalCommand::Open(name.to_string()),
            None => TerminalCommand::Unknown("open: missing filename".to_string()),
        },
        "rm" => match arg {
            Some((name, _)) => TerminalCommand::Rm(name.to_string()),
            None => TerminalCommand::Unknown("rm: missing filename".to_string()),
        },
        "cp" => match arg {
            Some((src, dst_rest)) if !dst_rest.is_empty() => match next_arg(dst_rest) {
                Ok(Some((dst, _))) => TerminalCommand::Cp(src.to_string(), dst.to_string()),
                Err(()) => TerminalCommand::Unknown(UNBALANCED_QUOTE.to_string()),
                _ => TerminalCommand::Unknown("cp: usage: cp <src> <dst>".to_string()),
            },
            _ => TerminalCommand::Unknown("cp: usage: cp <src> <dst>".to_string()),
        },
        "run" => TerminalCommand::Run,
        "clear" => TerminalCommand::Clear,
        "help" => TerminalCommand::Help,
        "" => TerminalCommand::Unknown(String::new()),
        other => TerminalCommand::Unknown(format!("unknown command: {other}")),
    }
}

/// Returns (common_prefix, all_matches) for the given prefix among candidates.
/// Returns None if no candidates match.
pub fn complete(prefix: &str, candidates: &[&str]) -> Option<(String, Vec<String>)> {
    let matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.starts_with(prefix))
        .map(|c| c.to_string())
        .collect();
    if matches.is_empty() {
        return None;
    }
    let common = &matches[0];
    let prefix_len = common
        .chars()
        .enumerate()
        .take_while(|&(i, c)| matches.iter().all(|m| m.chars().nth(i) == Some(c)))
        .count();
    let common_prefix = common[..common
        .chars()
        .take(prefix_len)
        .map(|c| c.len_utf8())
        .sum::<usize>()]
        .to_string();
    Some((common_prefix, matches))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ls() {
        assert!(matches!(parse_command("ls"), TerminalCommand::Ls));
    }

    #[test]
    fn parse_save_with_name() {
        match parse_command("save test.mkr") {
            TerminalCommand::Save(Some(name)) => assert_eq!(name, "test.mkr"),
            _ => panic!("expected Save(Some)"),
        }
    }

    #[test]
    fn parse_save_no_name() {
        assert!(matches!(parse_command("save"), TerminalCommand::Save(None)));
    }

    #[test]
    fn parse_open_with_name() {
        match parse_command("open hello") {
            TerminalCommand::Open(name) => assert_eq!(name, "hello"),
            _ => panic!("expected Open"),
        }
    }

    #[test]
    fn parse_rm() {
        match parse_command("rm file") {
            TerminalCommand::Rm(name) => assert_eq!(name, "file"),
            _ => panic!("expected Rm"),
        }
    }

    #[test]
    fn parse_cp() {
        match parse_command("cp src dst") {
            TerminalCommand::Cp(s, d) => {
                assert_eq!(s, "src");
                assert_eq!(d, "dst");
            }
            _ => panic!("expected Cp"),
        }
    }

    #[test]
    fn parse_run() {
        assert!(matches!(parse_command("run"), TerminalCommand::Run));
    }

    #[test]
    fn parse_clear() {
        assert!(matches!(parse_command("clear"), TerminalCommand::Clear));
    }

    #[test]
    fn parse_help() {
        assert!(matches!(parse_command("help"), TerminalCommand::Help));
    }

    #[test]
    fn parse_unknown() {
        assert!(matches!(parse_command("foo"), TerminalCommand::Unknown(_)));
    }

    #[test]
    fn parse_empty() {
        assert!(matches!(parse_command(""), TerminalCommand::Unknown(_)));
    }

    #[test]
    fn parse_whitespace_trimmed() {
        assert!(matches!(parse_command("  ls  "), TerminalCommand::Ls));
    }

    #[test]
    fn parse_unbalanced_quote() {
        match parse_command(r#"save "my file"#) {
            TerminalCommand::Unknown(msg) => assert_eq!(msg, "unbalanced quotes"),
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn parse_cp_unbalanced_quote() {
        match parse_command(r#"cp "my src dst"#) {
            TerminalCommand::Unknown(msg) => assert_eq!(msg, "unbalanced quotes"),
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn complete_single_match() {
        let (prefix, matches) = complete("sa", COMMAND_NAMES).unwrap();
        assert_eq!(prefix, "save");
        assert_eq!(matches, vec!["save"]);
    }

    #[test]
    fn complete_multiple_matches() {
        let (prefix, matches) = complete("c", COMMAND_NAMES).unwrap();
        assert_eq!(prefix, "c");
        assert_eq!(matches, vec!["clear", "cp"]);
    }

    #[test]
    fn complete_no_match() {
        assert!(complete("z", COMMAND_NAMES).is_none());
    }

    #[test]
    fn complete_empty_prefix() {
        let (_, matches) = complete("", COMMAND_NAMES).unwrap();
        assert_eq!(matches.len(), COMMAND_NAMES.len());
    }

    #[test]
    fn complete_exact_match() {
        let (prefix, matches) = complete("ls", COMMAND_NAMES).unwrap();
        assert_eq!(prefix, "ls");
        assert_eq!(matches, vec!["ls"]);
    }

    #[test]
    fn parse_open_unquoted_first_word() {
        match parse_command("open my file.mkr") {
            TerminalCommand::Open(name) => assert_eq!(name, "my"),
            _ => panic!("expected Open"),
        }
    }

    #[test]
    fn parse_open_quoted() {
        match parse_command(r#"open "my file.mkr""#) {
            TerminalCommand::Open(name) => assert_eq!(name, "my file.mkr"),
            _ => panic!("expected Open"),
        }
    }

    #[test]
    fn parse_rm_unquoted_first_word() {
        match parse_command("rm my file.mkr") {
            TerminalCommand::Rm(name) => assert_eq!(name, "my"),
            _ => panic!("expected Rm"),
        }
    }

    #[test]
    fn parse_rm_quoted() {
        match parse_command(r#"rm "my file.mkr""#) {
            TerminalCommand::Rm(name) => assert_eq!(name, "my file.mkr"),
            _ => panic!("expected Rm"),
        }
    }

    #[test]
    fn parse_save_quoted() {
        match parse_command(r#"save "my file.mkr""#) {
            TerminalCommand::Save(Some(name)) => assert_eq!(name, "my file.mkr"),
            _ => panic!("expected Save(Some)"),
        }
    }

    #[test]
    fn parse_cp_quoted() {
        match parse_command(r#"cp "my src" "my dst""#) {
            TerminalCommand::Cp(s, d) => {
                assert_eq!(s, "my src");
                assert_eq!(d, "my dst");
            }
            _ => panic!("expected Cp"),
        }
    }

    #[test]
    fn parse_cp_first_quoted() {
        match parse_command(r#"cp "my src" dst"#) {
            TerminalCommand::Cp(s, d) => {
                assert_eq!(s, "my src");
                assert_eq!(d, "dst");
            }
            _ => panic!("expected Cp"),
        }
    }

    #[test]
    fn complete_filenames() {
        let files = &["game.mkr", "game2.mkr", "hello.mkr"];
        let (prefix, matches) = complete("game", files).unwrap();
        assert_eq!(prefix, "game");
        assert_eq!(matches, vec!["game.mkr", "game2.mkr"]);
    }
}
