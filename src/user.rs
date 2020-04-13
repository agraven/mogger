use bcrypt::BcryptError;
use chrono::{Duration, NaiveDateTime, Utc};
use cookie::CookieJar;
use diesel::{prelude::*, result::Error as DieselError};
use diesel_derive_enum::DbEnum;
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

use std::borrow::Cow;

use crate::{
    schema::{groups, sessions, users},
    Connection, DbConnection,
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
    /// The group the user belongs to
    group: String,
}

impl User {
    /// Verify the supplied password matches the users
    pub fn verify(&self, password: &str) -> Result<bool, BcryptError> {
        verify(password, &self.salt, &self.hash)
    }

    /// Checks if a user has a given permission.
    pub fn allowed(
        &self,
        permission: Permission,
        connection: &Connection,
    ) -> Result<bool, DieselError> {
        use crate::schema::groups::dsl;

        let group: Group = dsl::groups.find(&self.group).first(connection)?;
        Ok(group.permissions.contains(&permission) || group.permissions.contains(&Permission::All))
    }

    pub fn editable(
        &self,
        session: Option<&Session>,
        conn: &Connection,
    ) -> Result<bool, DieselError> {
        if let Some(session) = session {
            Ok(session.allowed(Permission::EditForeignUser, conn)? || session.user == self.id)
        } else {
            Ok(false)
        }
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
    #[serde(default = "default_group")]
    group: String,
    /// Venus fly trap for spam bots
    pub secret: String,
}

fn default_group() -> String {
    String::from("default")
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
            group: self.group,
        }
    }
}

#[derive(AsChangeset, Deserialize, Serialize)]
#[table_name = "users"]
pub struct UserProfile {
    pub name: String,
    pub email: String,
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

#[derive(Serialize, Deserialize)]
pub struct PasswordChange<'a> {
    #[serde(borrow)]
    old: Cow<'a, str>,
    #[serde(borrow)]
    new: Cow<'a, str>,
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

    pub fn user(&self, connection: &Connection) -> Result<User, DieselError> {
        get(connection, &self.user)
    }

    pub fn allowed(
        &self,
        permission: Permission,
        connection: &Connection,
    ) -> Result<bool, DieselError> {
        self.user(connection)?.allowed(permission, connection)
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
            let connection = DbConnection::borrow_from(&state).lock()?;
            let cookie = CookieJar::borrow_from(&state)
                .get("session")
                .map(|cookie| cookie.value());
            if let Some(id) = cookie {
                if let Some(session) = Session::from_id(id, &connection)? {
                    std::mem::drop(connection);
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

pub fn logout(connection: &Connection, session: &str) -> Result<usize, DieselError> {
    use crate::schema::sessions::dsl;

    diesel::delete(dsl::sessions.find(session)).execute(connection)
}

pub fn edit_profile(
    connection: &Connection,
    id: &str,
    profile: &UserProfile,
) -> Result<usize, DieselError> {
    use crate::schema::users::dsl;

    diesel::update(dsl::users.find(id))
        .set(profile)
        .execute(connection)
}

pub fn change_password(
    connection: &Connection,
    id: &str,
    change: &PasswordChange,
) -> Result<bool, failure::Error> {
    use crate::schema::users::dsl;

    let (old_hash, salt): (String, Vec<u8>) = dsl::users
        .select((dsl::hash, dsl::salt))
        .find(id)
        .first(connection)?;

    // Verify password
    if !verify(&change.old, &salt, &old_hash)? {
        return Ok(false);
    }

    // Get new hash and salt
    let new_salt: Box<[u8]> = Box::new(generate_salt());
    let new_salt = new_salt.into_vec();
    let new_hash = hash(&change.new, &new_salt)?;

    // Write new values to database
    diesel::update(dsl::users.find(id))
        .set((dsl::hash.eq(&new_hash), dsl::salt.eq(&new_salt)))
        .execute(connection)?;
    Ok(true)
}

#[derive(Serialize, Deserialize)]
pub struct UserDeletion<'a> {
    #[serde(borrow)]
    password: Cow<'a, str>,
    #[serde(default)]
    pub purge: bool,
}

pub fn delete(
    connection: &Connection,
    id: &str,
    deletion: &UserDeletion,
) -> Result<(), failure::Error> {
    use crate::schema::comments::dsl as c;
    use crate::schema::sessions::dsl as s;
    use crate::schema::users::dsl as u;

    // Verify password
    // TODO: turn this select into a function
    let (salt, hash): (Vec<u8>, String) = u::users
        .select((u::salt, u::hash))
        .find(id)
        .first(connection)?;
    if !verify(&deletion.password, &salt, &hash)? {
        return Err(failure::err_msg("Wrong password"));
    }

    // We have to make this variable because types can't be inferred for None
    let none_str: Option<String> = None;
    if deletion.purge {
        // purge contents of user's comments
        diesel::update(c::comments.filter(c::author.eq(&id)))
            .set((
                c::author.eq(none_str),
                c::name.eq("[deleted]"),
                c::content.eq(""),
            ))
            .execute(connection)?;
    } else {
        // Remove ownership for user's comments
        diesel::update(c::comments.filter(c::author.eq(&id)))
            .set((c::author.eq(none_str), c::name.eq("[deleted]")))
            .execute(connection)?;
    }
    // Delete all sessions
    diesel::delete(s::sessions.filter(s::user.eq(&id))).execute(connection)?;

    diesel::delete(u::users.find(id)).execute(connection)?;
    Ok(())
}

pub fn count(connection: &Connection) -> Result<i64, DieselError> {
    use crate::schema::users::dsl::*;

    users.count().first(connection)
}

#[derive(Clone, Debug, Queryable, Identifiable, Insertable)]
#[table_name = "groups"]
pub struct Group {
    id: String,
    permissions: Vec<Permission>,
}

/*impl Queryable<groups::SqlType, diesel::pg::Pg> for Group {
    type Row = (String, Vec<Permission>);

    fn build(row: Self::Row) -> Self {
        Group {
            name: row.0,
            permissions: row.1.iter().copied().collect(),
        }
    }
}

impl<DB> ToSql<diesel::types::Array<PermissionMapping, DB>> for BTreeSet<Permission>
where
    DB: diesel::backend::Backend
{
    fn to_sql<W: Write>(&self, out: &mut )
}*/

/// Represents a type of action that a user or group can be allowed or denied permission for
#[derive(Clone, Copy, Debug, PartialEq, Eq, DbEnum)]
pub enum Permission {
    All,

    CreateArticle,
    EditArticle,
    DeleteArticle,
    EditForeignArticle,
    DeleteForeignArticle,

    CreateComment,
    EditComment,
    DeleteComment,
    EditForeignComment,
    DeleteForeignComment,

    CreateUser,
    EditForeignUser,
    DeleteForeignUser,
}

/* turns out enums are feasible so i'm dropping the to/from text conversion
impl Permission {
    /// Gets a permission from its string representation
    pub fn from_name(name: &str) -> Option<Self> {
        use Permission::*;
        match name {
            "create_article" => CreateArticle,
            "edit_article" => EditArticle,
            "delete_article" => DeleteArticle,
            "edit_foreign_article" => EditForeignArticle,
            "delete_foreign_article" => DeleteForeignArticle,

            "create_comment" => CreateComment,
            "edit_comment" => EditComment,
            "delete_comment" => DeleteComment,
            "edit_foreign_comment" => EditForeignComment,
            "delete_foreign_comment" => DeleteForeignComment,

            "create_user" => CreateUser,
            "edit_foreign_user" => EditForeignUser,
            "delete_foreign_user" => DeleteForeignUser,

            _ => return None,
        }.into()
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Permission::*;
        let string = match *self {
            CreateArticle => "create_article",
            EditArticle => "edit_article",
            DeleteArticle => "delete_article",
            EditForeignArticle => "edit_foreign_article",
            DeleteForeignArticle => "delete_foreign_article",

            CreateComment => "create_comment",
            EditComment => "edit_comment",
            DeleteComment => "delete_comment",
            EditForeignComment => "edit_foreign_comment",
            DeleteForeignComment => "delete_foreign_comment",

            CreateUser => "create_user",
            EditForeignUser => "edit_foreign_user",
            DeleteForeignUser => "delete_foreign_user",
        };

        write!(f, "{}", string)
    }
}*/
