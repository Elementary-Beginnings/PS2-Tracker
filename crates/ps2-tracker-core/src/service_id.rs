//! Which Census service ID the app uses, and where it comes from.
//!
//! Precedence: environment variable `PS2_SERVICE_ID` > configuration file
//! (Settings) > the default built into official builds > `example`.
//!
//! Official builds embed a project service ID at compile time
//! (`PS2_DEFAULT_SERVICE_ID`, a masked CI variable). That value is readable
//! from the binary; it grants no account access and only identifies the app
//! to Daybreak (attribution, rate limiting). It is still never written to the
//! configuration file and never shown: [`ServiceId`] prints only its source
//! (`Display`/`Debug`), and the UI only learns the source.

use std::fmt;

/// Compile-time default; unset or empty in builds without the CI variable.
const BUILT_IN: Option<&str> = option_env!("PS2_DEFAULT_SERVICE_ID");

/// The value Census accepts without registration (very few requests).
pub const EXAMPLE: &str = "example";

/// Letters, digits and `_`, not empty (also rules out a leading `s:`).
pub const fn is_valid(id: &str) -> bool {
    let b = id.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if !(c.is_ascii_alphanumeric() || c == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

// An invalid build value stops the build. The message names the variable,
// never the value.
const _: () = {
    if let Some(v) = BUILT_IN
        && !v.is_empty()
        && !is_valid(v)
    {
        panic!("PS2_DEFAULT_SERVICE_ID may only contain letters, digits and _ (without \"s:\")");
    }
};

/// The built-in default, if this build has one.
pub fn built_in() -> Option<&'static str> {
    match BUILT_IN {
        Some(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Where the active service ID comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `PS2_SERVICE_ID`
    Environment,
    /// The configuration file / Settings.
    Config,
    /// Embedded at compile time (official builds).
    BuiltIn,
    /// Neither of the above: Census' public `example` ID.
    Example,
}

impl Source {
    /// Stable name for the UI and the logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Environment => "environment",
            Source::Config => "config",
            Source::BuiltIn => "builtin",
            Source::Example => "example",
        }
    }
}

/// The resolved service ID. `Display` and `Debug` show only the source.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceId {
    value: String,
    source: Source,
}

impl ServiceId {
    /// Resolves the active service ID from the process environment, the
    /// configured value (empty = not set) and the built-in default.
    pub fn resolve(configured: &str) -> Result<Self, String> {
        let env = std::env::var("PS2_SERVICE_ID").ok();
        Self::resolve_from(env.as_deref(), configured, built_in())
    }

    /// [`resolve`](Self::resolve) with explicit inputs (tests).
    pub fn resolve_from(
        env: Option<&str>,
        configured: &str,
        built_in: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(e) = env.map(str::trim).filter(|e| !e.is_empty()) {
            let e = e.strip_prefix("s:").unwrap_or(e);
            if !is_valid(e) {
                return Err(
                    "PS2_SERVICE_ID may only contain letters, digits and _ (without \"s:\")".into(),
                );
            }
            return Ok(Self::new(e, Source::Environment));
        }
        if !configured.is_empty() {
            return Ok(Self::new(configured, Source::Config));
        }
        match built_in {
            Some(b) if is_valid(b) => Ok(Self::new(b, Source::BuiltIn)),
            _ => Ok(Self::new(EXAMPLE, Source::Example)),
        }
    }

    fn new(value: &str, source: Source) -> Self {
        Self {
            value: value.to_owned(),
            source,
        }
    }

    /// The value for the Census URLs. Do not log or display it.
    pub fn expose(&self) -> &str {
        &self.value
    }

    pub fn source(&self) -> Source {
        self.source
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.source {
            Source::Environment => "PS2_SERVICE_ID",
            Source::Config => "configured service ID",
            Source::BuiltIn => "built-in default",
            Source::Example => "example",
        })
    }
}

impl fmt::Debug for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceId({})", self.source.as_str())
    }
}

/// Replaces every occurrence of `secret` in `text` (error messages).
pub fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Option<&str> = Some("builtin0secret");

    #[test]
    fn precedence() {
        let r = |e, c, b| ServiceId::resolve_from(e, c, b).unwrap();
        let x = r(Some("envid"), "cfgid", B);
        assert_eq!((x.expose(), x.source()), ("envid", Source::Environment));
        let x = r(Some("s:envid"), "", None);
        assert_eq!(x.expose(), "envid");
        let x = r(Some(""), "cfgid", B);
        assert_eq!((x.expose(), x.source()), ("cfgid", Source::Config));
        let x = r(None, "", B);
        assert_eq!(
            (x.expose(), x.source()),
            ("builtin0secret", Source::BuiltIn)
        );
        let x = r(None, "", None);
        assert_eq!((x.expose(), x.source()), ("example", Source::Example));
        // an empty build value counts as "no default"
        let x = r(None, "", Some(""));
        assert_eq!(x.source(), Source::Example);
    }

    #[test]
    fn invalid_environment_value_is_an_error_without_the_value() {
        let e = ServiceId::resolve_from(Some("bad/val"), "", B).unwrap_err();
        assert!(!e.contains("bad/val"), "{e}");
    }

    #[test]
    fn display_and_debug_never_show_the_value() {
        let x = ServiceId::resolve_from(None, "", B).unwrap();
        assert_eq!(x.to_string(), "built-in default");
        let dbg = format!("{x:?} {:?}", Some(&x));
        assert!(!dbg.contains("builtin0secret"), "{dbg}");
        let c = ServiceId::resolve_from(None, "cfg0secret", B).unwrap();
        assert!(!format!("{c} {c:?}").contains("cfg0secret"));
    }

    #[test]
    fn validation() {
        assert!(is_valid("abc_123"));
        assert!(!is_valid(""));
        assert!(!is_valid("s:abc"));
        assert!(!is_valid("a b"));
        assert!(!is_valid("ä"));
    }

    #[test]
    fn redaction() {
        assert_eq!(redact("x secret y secret", "secret"), "x *** y ***");
        assert_eq!(redact("abc", ""), "abc");
    }
}
