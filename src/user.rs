use bcrypt::BcryptError;
use chrono::{Duration, NaiveDateTime, Utc};
use cookie::CookieJar;
use diesel::{prelude::*, result::Error as DieselError, PgConnection as Connection};
use futures::future;
use gotham::{
    handler::HandlerFuture,
    helpers::http::response::create_response,
    middleware::Middleware,
    state::{FromState, State},
};
use gotham_derive::{NewMiddleware, StateData};
use rand::prelude::*;
use sha2::{Digest, Sha256};

use crate::{
    schema::{sessions, users},
    DbConnection,
};

const SALT_LEN: usize = 16;
const SESSION_LEN: usize = 24;

#[derive(Debug, Deserialize, Serialize, Queryable, Identifiable, Insertable)]
pub struct User {
    /// The unique username/login
    pub id: String,
    /// The hashed password
    hash: String,
    /// The salt for the password
    salt: Vec<u8>,
    /// The user's display name
    pub name: String,
    /// The user's email address
    pub email: String,
}

impl User {
    /// Verify the supplied password matches the users
    pub fn verify(&self, password: &str) -> Result<bool, BcryptError> {
        verify(password, &self.salt, &self.hash)
    }
}

/// A to be created user.
///
/// NOTE: This structure contains the user's unencrypted password, handle it with great care!
#[derive(Clone, Deserialize, Serialize)]
pub struct NewUser {
    /// The username
    id: String,
    /// The users raw password
    password: String,
    /// The user's display name
    name: String,
    /// The user's email address
    email: String,
}

impl NewUser {
    /// Converts the structure into a proper user, generating a salt and hashing the password.
    pub fn into_user(self) -> User {
        let salt: Box<[u8]> = Box::new(generate_salt());
        User {
            id: self.id,
            hash: hash(&self.password, &salt).unwrap(),
            salt: salt.into_vec(),
            name: self.name,
            email: self.email,
        }
    }
}

/// Login credentials
#[derive(Deserialize)]
pub struct Login {
    user: String,
    password: String,
}

impl Login {
    /// Create a session if username and password is valid
    pub fn login(&self, connection: &Connection) -> Result<Option<Session>, failure::Error> {
        let user: Option<User> = users::dsl::users
            .find(&self.user)
            .first(connection)
            .optional()?;
        match user {
            Some(ref user) if user.verify(&self.password)? => {
                let session = Session::new(&self.user);
                diesel::insert_into(sessions::table)
                    .values(&session)
                    .execute(connection)?;
                Ok(Some(session))
            }
            _ => Ok(None),
        }
    }
}

impl From<NewUser> for Login {
    fn from(u: NewUser) -> Self {
        Self {
            user: u.id,
            password: u.password,
        }
    }
}

#[derive(Clone, Queryable, Insertable, Serialize, StateData)]
pub struct Session {
    pub id: String,
    pub user: String,
    pub expires: NaiveDateTime,
}

impl Session {
    /// Generates a new session.
    ///
    /// NB: Must be inserted into the database for the session to be valid.
    pub fn new(user: &str) -> Session {
        // Fill array with random data
        let mut id = [0u8; SESSION_LEN];
        StdRng::from_entropy().fill(&mut id[..]);
        Session {
            id: base64::encode(&id),
            user: user.to_owned(),
            expires: Utc::now().naive_utc() + Duration::days(30),
        }
    }

    /// Get the session with the specified id
    pub fn from_id(id: &str, connection: &Connection) -> Result<Option<Session>, DieselError> {
        sessions::dsl::sessions
            .find(id)
            .first(connection)
            .optional()
    }
}

#[derive(Clone, NewMiddleware)]
pub struct SessionMiddleware;

impl Middleware for SessionMiddleware {
    fn call<C>(self, mut state: State, chain: C) -> Box<HandlerFuture>
    where
        C: FnOnce(State) -> Box<HandlerFuture>,
    {
        let put_session = |state: &mut State| -> Result<(), failure::Error> {
            let arc = DbConnection::borrow_from(&state).get();
            let connection = &arc.lock().or(Err(failure::err_msg("async error")))?;
            let cookie = CookieJar::borrow_from(&state)
                .get("session")
                .map(|cookie| cookie.value());
            if let Some(id) = cookie {
                if let Some(session) = Session::from_id(id, connection)? {
                    state.put(session);
                }
            }
            Ok(())
        };
        match put_session(&mut state) {
            Ok(()) => Box::new(chain(state)),
            Err(e) => {
                let response = create_response(
                    &state,
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    mime::TEXT_PLAIN,
                    e.to_string(),
                );
                Box::new(future::ok((state, response)))
            }
        }
    }
}

/// Password hashing function. Inspired by [Dropbox's password storage policy][1].
///
/// First the password and salt are combined, then hashed with SHA256 to prevent DoS attacks. The
/// password is then hashed with bcrypt.
///
/// [1]: https://blogs.dropbox.com/tech/2016/09/how-dropbox-securely-stores-your-passwords/
fn hash(key: &str, salt: &[u8]) -> Result<String, BcryptError> {
    // digest the password and salt
    let digest = Sha256::new().chain(key).chain(salt).result();
    // Hash the password with bcrypt (base64 encode to avoid zero-bytes).
    let hash = bcrypt::hash(base64::encode(&digest), bcrypt::DEFAULT_COST)?;
    Ok(hash)
}

fn verify(key: &str, salt: &[u8], hash: &str) -> Result<bool, BcryptError> {
    let digest = Sha256::new().chain(key).chain(salt).result();
    let matches = bcrypt::verify(&base64::encode(&digest), hash)?;
    Ok(matches)
}

/// Generates a new salt of length `SALT_LEN`
fn generate_salt() -> [u8; SALT_LEN] {
    let mut bytes = [0u8; SALT_LEN];

    StdRng::from_entropy().fill(&mut bytes[..]);

    bytes
}

/// Creates a user
pub fn create(connection: &Connection, user: NewUser) -> Result<usize, DieselError> {
    diesel::insert_into(users::table)
        .values(&user.into_user())
        .execute(connection)
}

pub fn get(connection: &Connection, id: &str) -> Result<User, DieselError> {
    use crate::schema::users::dsl;

    dsl::users.find(id).first(connection)
}
