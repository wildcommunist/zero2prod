use secrecy::Secret;

pub struct UserPassword(Secret<String>);
