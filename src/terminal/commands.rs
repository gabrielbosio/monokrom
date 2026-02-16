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
        Some("clear") => TerminalCommand::Clear,
        Some("help") => TerminalCommand::Help,
        Some(other) => TerminalCommand::Unknown(format!("unknown command: {other}")),
        None => TerminalCommand::Unknown(String::new()),
    }
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
}
