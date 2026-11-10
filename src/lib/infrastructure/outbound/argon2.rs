/// The password hasher adapter.
mod password_hasher;

pub(crate) use self::password_hasher::Argon2PasswordHasher;
