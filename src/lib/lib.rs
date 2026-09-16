pub mod application;
pub mod domain;
pub mod infrastructure;

/// Fixtures shared by the in-crate test modules.
#[cfg(test)]
mod test_helpers {
    /// A password that satisfies the password policy.
    ///
    /// Use it when a test only needs a valid password: registering a user,
    /// authenticating, or provisioning a credential. Tests whose subject is
    /// password behaviour (policy validation, wrong-password rejection,
    /// change-password, hashing, verification) keep an explicit password.
    pub const SECRET_PASSWORD: &str = "C0rrect!Horse";
}
