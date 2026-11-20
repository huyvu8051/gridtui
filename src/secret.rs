//! Connection passwords in the OS keychain (service `gridtui`, account = connection id).
use anyhow::Result;
use keyring::{Entry, Error};
use uuid::Uuid;

const SERVICE: &str = "gridtui";

fn entry(id: Uuid) -> keyring::Result<Entry> {
    Entry::new(SERVICE, &id.to_string())
}

/// The saved password, or "" if none is saved.
pub fn get_password(id: Uuid) -> Result<String> {
    match entry(id)?.get_password() {
        Ok(p) => Ok(p),
        Err(Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Save `password`; an empty one removes the saved password.
pub fn set_password(id: Uuid, password: &str) -> Result<()> {
    if password.is_empty() {
        return delete_password(id);
    }
    Ok(entry(id)?.set_password(password)?)
}

pub fn delete_password(id: Uuid) -> Result<()> {
    match entry(id)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
