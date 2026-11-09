//! Reading secrets at egress time, and the process environment they come from.
//!
//! A secret is resolved for the lifetime of one attempt and is never stored,
//! logged or put in an evidence row. `Debug` here is hand-written for exactly
//! that reason: a derived one would print the bytes.

/// One resolved secret. Deliberately opaque: it has no public accessor except
/// the bearer rendering the transport needs, and a redacting `Debug`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// The `Authorization` header value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }

    /// The raw bytes, for the client-credential pass-through case.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// A secret built from a resolved value.
    pub fn from_value(value: &str) -> Self {
        Secret(value.to_owned())
    }

    /// A secret built from a literal, for tests that must not touch the
    /// environment or a file.
    #[cfg(test)]
    pub fn debug_value(value: &str) -> Self {
        Secret::from_value(value)
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::config::{CommandLine, SecretRef};
    use crate::gate::refusal::code;

    fn command_line() -> CommandLine {
        let mut command_line = CommandLine::default();
        command_line.env.insert(String::from("K1"), String::from("sk-1"));
        command_line.env.insert(String::from("K_EMPTY"), String::new());
        command_line
    }

    #[test]
    fn an_env_secret_resolves() {
        let secret = command_line()
            .read_secret(&SecretRef::Env(String::from("K1")), false, None)
            .expect("resolves");
        assert_eq!(secret.bearer(), "Bearer sk-1");
    }

    #[test]
    fn an_unset_or_empty_env_is_named_without_its_value() {
        for name in ["K_MISSING", "K_EMPTY"] {
            let error = command_line()
                .read_secret(&SecretRef::Env(String::from(name)), false, None)
                .expect_err("must fail");
            assert!(error.contains(name));
            assert!(error.contains(code::SECRET_UNREADABLE));
            assert!(!error.contains("sk-1"));
        }
    }

    #[test]
    fn a_client_credential_needs_the_caller_header() {
        let error = command_line()
            .read_secret(&SecretRef::Env(String::new()), true, None)
            .expect_err("must fail");
        assert!(error.contains("x-gate-upstream-authorization"));
        let secret = command_line()
            .read_secret(
                &SecretRef::Env(String::new()),
                true,
                Some(String::from("Bearer caller")),
            )
            .expect("resolves");
        assert_eq!(secret.expose(), "Bearer caller");
    }

    #[test]
    fn oauth_and_command_shapes_are_not_in_this_slice() {
        for secret in [
            SecretRef::OauthFile(String::from("t.json")),
            SecretRef::Command(vec![String::from("helper")]),
        ] {
            let error = command_line().read_secret(&secret, false, None).expect_err("must fail");
            assert!(error.contains(code::NOT_IN_THIS_SLICE));
        }
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let secret = command_line()
            .read_secret(&SecretRef::Env(String::from("K1")), false, None)
            .expect("resolves");
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "Secret(<redacted>)");
        assert!(!rendered.contains("sk-1"));
    }

    #[test]
    fn a_file_secret_is_read_and_trimmed() {
        let dir = std::env::temp_dir().join(format!("gate-secret-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("k.txt");
        std::fs::write(&path, "sk-file\r\n").expect("write");
        let secret = command_line()
            .read_secret(&SecretRef::File(path.to_string_lossy().into_owned()), false, None)
            .expect("resolves");
        assert_eq!(secret.expose(), "sk-file");
        std::fs::remove_dir_all(&dir).ok();
    }
}
