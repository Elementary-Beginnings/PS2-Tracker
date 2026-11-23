//! Where the refresh token lives: only in the operating system's keyring
//! (Linux: Secret Service, e.g. GNOME Keyring or KWallet; Windows: Credential
//! Manager). There is deliberately no file fallback: without a keyring the
//! sign-in is not kept and the UI says so.
//!
//! Errors never carry the secret: keyring errors that hold the stored bytes
//! (`BadEncoding`, `BadDataFormat`) are mapped to a fixed text.

use std::sync::{Arc, Mutex};

/// Keyring "service" of the entry.
pub const SERVICE: &str = "dev.herz.ps2tracker.online-sync";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// No keyring on this system, it is locked or access was refused.
    #[error("no keyring available: {0}")]
    Unavailable(String),
    /// Any other keyring failure.
    #[error("keyring: {0}")]
    Failed(String),
}

/// A place for one secret (blocking calls: run them off the async runtime).
pub trait SecretStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, SecretError>;
    fn set(&self, value: &str) -> Result<(), SecretError>;
    fn delete(&self) -> Result<(), SecretError>;
    /// Checks that the store can be used (a read that finds nothing is fine).
    fn probe(&self) -> Result<(), SecretError> {
        self.get().map(|_| ())
    }
}

/// In memory only (tests).
#[derive(Default)]
pub struct MemorySecret {
    value: Mutex<Option<String>>,
    /// Simulates a system without keyring.
    pub unavailable: bool,
}

impl MemorySecret {
    pub fn unavailable() -> Self {
        Self {
            value: Mutex::new(None),
            unavailable: true,
        }
    }
    fn check(&self) -> Result<(), SecretError> {
        if self.unavailable {
            Err(SecretError::Unavailable("test".into()))
        } else {
            Ok(())
        }
    }
}

impl SecretStore for MemorySecret {
    fn get(&self) -> Result<Option<String>, SecretError> {
        self.check()?;
        Ok(self.value.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }
    fn set(&self, value: &str) -> Result<(), SecretError> {
        self.check()?;
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = Some(value.to_owned());
        Ok(())
    }
    fn delete(&self) -> Result<(), SecretError> {
        self.check()?;
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
}

/// The platform keyring through `keyring-core`. `account` names the entry
/// (one per API address, so a test server never overwrites the real login).
/// The keyring is contacted on first use only (the Secret Service store
/// connects to D-Bus when it is created), never while the sync is off. A
/// failed connection is tried again on the next use (keyring started later).
pub struct KeyringSecret {
    store: Mutex<Option<Arc<keyring_core::CredentialStore>>>,
    account: String,
}

impl KeyringSecret {
    pub fn new(account: &str) -> Self {
        Self {
            store: Mutex::new(None),
            account: account.to_owned(),
        }
    }

    fn entry(&self) -> Result<keyring_core::Entry, SecretError> {
        let store = {
            let mut s = self.store.lock().unwrap_or_else(|e| e.into_inner());
            match s.as_ref() {
                Some(st) => st.clone(),
                None => {
                    let st = platform_store()?;
                    *s = Some(st.clone());
                    st
                }
            }
        };
        keyring_core::Entry::new_in_store(&store, SERVICE, &self.account).map_err(map_err)
    }
}

impl SecretStore for KeyringSecret {
    fn get(&self) -> Result<Option<String>, SecretError> {
        match self.entry()?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }
    fn set(&self, value: &str) -> Result<(), SecretError> {
        self.entry()?.set_password(value).map_err(map_err)
    }
    fn delete(&self) -> Result<(), SecretError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

#[cfg(target_os = "linux")]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, SecretError> {
    let s: Arc<keyring_core::CredentialStore> =
        zbus_secret_service_keyring_store::Store::new().map_err(map_err)?;
    Ok(s)
}

#[cfg(windows)]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, SecretError> {
    let s: Arc<keyring_core::CredentialStore> =
        windows_native_keyring_store::Store::new().map_err(map_err)?;
    Ok(s)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, SecretError> {
    Err(SecretError::Unavailable(
        "no supported keyring on this platform".into(),
    ))
}

/// Keyring error → text without stored data.
fn map_err(e: keyring_core::Error) -> SecretError {
    use keyring_core::Error as E;
    match e {
        E::NoStorageAccess(p) => SecretError::Unavailable(p.to_string()),
        E::NoDefaultStore => SecretError::Unavailable("no keyring".into()),
        E::PlatformFailure(p) => SecretError::Unavailable(p.to_string()),
        E::BadEncoding(_) | E::BadDataFormat(..) => {
            SecretError::Failed("stored value is unreadable".into())
        }
        E::NoEntry => SecretError::Failed("no entry".into()),
        E::BadStoreFormat(s) => SecretError::Failed(s),
        E::TooLong(name, _) => SecretError::Failed(format!("{name} too long")),
        E::Invalid(name, why) => SecretError::Failed(format!("{name}: {why}")),
        E::Ambiguous(_) => SecretError::Failed("more than one entry".into()),
        _ => SecretError::Failed("unsupported operation".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store() {
        let m = MemorySecret::default();
        assert_eq!(m.get().unwrap(), None);
        m.set("abc").unwrap();
        assert_eq!(m.get().unwrap().as_deref(), Some("abc"));
        m.delete().unwrap();
        assert_eq!(m.get().unwrap(), None);
        let u = MemorySecret::unavailable();
        assert!(matches!(u.probe(), Err(SecretError::Unavailable(_))));
        assert!(u.set("x").is_err());
    }

    #[test]
    fn errors_with_stored_bytes_do_not_show_them() {
        let e = map_err(keyring_core::Error::BadEncoding(b"secret-token".to_vec()));
        assert!(!e.to_string().contains("secret-token"), "{e}");
    }
}
