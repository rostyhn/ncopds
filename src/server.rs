use keyring::{Entry, Error};
use serde_derive::{Deserialize, Serialize};
use std::fmt;
use url::Url;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Server {
    pub username: Option<String>,
    /// the url for the opds catalog, NOT just the domain name i.e https://example.com/opds
    pub base_url: Url,
}

/// Stores a password for a server in the system keychain.
///
/// # Arguments
///
/// * `s` - Server credentials to store the password for.
/// * `pwd` - Password to store.
///
pub fn store_password(s: &Server, pwd: &Option<String>) {
    match pwd {
        Some(p) => match &s.username {
            Some(u) => {
                let entry = Entry::new("ncopds", &format!("{}@{}", &u, s.base_url)).unwrap();
                entry.set_password(p).expect("failed to set password entry");
            }
            None => {}
        },
        None => {}
    }
}

impl Server {
    /// Returns the scheme + domain as a URL type.
    ///
    /// # Examples
    ///
    /// ```
    /// "https://example.com/path/further/down" -> "https://example.com"
    /// ```
    pub fn get_domain(&self) -> Url {
        // test
        Url::parse(&format!(
            "{0}://{1}",
            self.base_url.scheme(),
            self.base_url.domain().unwrap()
        ))
        .unwrap()
    }

    /// Retrieves the password for the username and server from the system's keychain. Servers
    /// without usernames do not have passwords associated with them.
    ///
    /// # Errors
    ///
    /// Errors can get thrown if the password has not been stored in the keyring before.
    ///
    pub fn get_password(&self) -> Result<Option<String>, Error> {
        // test
        match &self.username {
            Some(u) => {
                let entry = Entry::new("ncopds", &format!("{}@{}", &u, self.base_url)).unwrap();
                let password = entry.get_password()?;

                if password.is_empty() {
                    return Ok(None);
                }

                Ok(Some(password))
            }
            // no username means we don't need to store a password
            None => Ok(None),
        }
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "URL: {}\n USER: {:?}\n", self.base_url, self.username)
    }
}
