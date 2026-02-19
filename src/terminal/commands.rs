pub const COMMAND_NAMES: &[&str] = &[
    "clear", "cp", "help", "lex", "ls", "new", "open", "parse", "rm", "run", "save",
];

pub enum TerminalCommand {
    Ls,
    New,
    Save(Option<String>),
    Open(String),
    Rm(String),
    Cp(String, String),
    Run,
    Lex,
    Parse,
    Clear,
    Help,
    Unknown(String),
}

pub fn parse_command(input: &str) -> TerminalCommand {
    let parts: Vec<&str> = input.trim().splitn(3, ' ').collect();
    match parts.first().copied() {
        Some("ls") => TerminalCommand::Ls,
        Some("new") => TerminalCommand::New,
        Some("save") => TerminalCommand::Save(parts.get(1).map(|s| s.to_string())),
        Some("open") => match parts.get(1) {
            Some(name) => TerminalCommand::Open(name.to_string()),
            None => TerminalCommand::Unknown("open: missing filename".to_string()),
        },
        Some("rm") => match parts.get(1) {
            Some(name) => TerminalCommand::Rm(name.to_string()),
            None => TerminalCommand::Unknown("rm: missing filename".to_string()),
        },
        Some("cp") => match (parts.get(1), parts.get(2)) {
            (Some(src), Some(dst)) => TerminalCommand::Cp(src.to_string(), dst.to_string()),
            _ => TerminalCommand::Unknown("cp: usage: cp <src> <dst>".to_string()),
        },
        Some("run") => TerminalCommand::Run,
        Some("lex") => TerminalCommand::Lex,
        Some("parse") => TerminalCommand::Parse,
        Some("clear") => TerminalCommand::Clear,
        Some("help") => TerminalCommand::Help,
        Some(other) => TerminalCommand::Unknown(format!("unknown command: {other}")),
        None => TerminalCommand::Unknown(String::new()),
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
    fn complete_filenames() {
        let files = &["game.mkr", "game2.mkr", "hello.mkr"];
        let (prefix, matches) = complete("game", files).unwrap();
        assert_eq!(prefix, "game");
        assert_eq!(matches, vec!["game.mkr", "game2.mkr"]);
    }
}
