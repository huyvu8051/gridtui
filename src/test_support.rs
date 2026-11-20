//! Helpers shared by tests.

/// Everything that touches disk goes to a throwaway HOME, and passwords to keyring's
/// in-memory mock instead of the real keychain. Tests holding the returned guard run
/// one at a time and start with empty storage.
pub fn isolated_storage() -> std::sync::MutexGuard<'static, ()> {
    let guard = tuikit::testing::isolated_storage(crate::APP);
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    guard
}
