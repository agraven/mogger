//! Module for login, signup and user settings

use askama::Template;
use cookie::{Cookie, SameSite};
use gotham::{
    helpers::http::response::create_temporary_redirect as temp_redirect,
    hyper::{header, StatusCode},
    state::{client_addr, FromState, State},
};

use crate::{
    comment,
    config::Settings,
    db::{Connection, DbConnection},
    document::{article::CommentTemplate, DocumentResult, TemplateExt},
    handler::users::UserPath,
    user::{
        self, Login, NewUser, PasswordChange, Permission, Session, User, UserDeletion, UserProfile,
    },
};

fn session_cookie<'a>(state: &State, id: &str) -> Cookie<'a> {
    let settings = Settings::borrow_from(state);
    let mut cookie = Cookie::build("session", id.to_owned())
        .same_site(SameSite::Strict)
        .http_only(true)
        .finish();
    if settings.cookie.secure {
        cookie.set_secure(true);
    }
    if let Some(ref domain) = settings.cookie.domain {
        cookie.set_domain(domain.to_owned());
    }
    cookie
}

#[derive(Template, Clone)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

#[derive(Template, Clone)]
#[template(path = "login-result.html")]
pub struct LoginResultTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

/// Login form
pub fn login(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    Ok(LoginTemplate {
        session: Session::try_borrow_from(state),
        connection,
    }
    .to_response(state))
}

/// Login post. Sets session cookie if login was successful.
pub fn login_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let credentials: Login = serde_urlencoded::from_bytes(&post)?;
    let new_session = credentials.login(connection)?;

    let mut response = LoginResultTemplate {
        session: new_session.as_ref(),
        connection,
    }
    .to_response(state);

    // Set session cookie if login was successful
    if let Some(session) = new_session {
        let cookie = session_cookie(state, &session.id);
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie.to_string().parse()?);
    }

    Ok(response)
}

#[derive(Template)]
#[template(path = "signup.html")]
struct SignupTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    signup_enabled: bool,
}

pub fn signup(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let signup_enabled = Settings::borrow_from(state).features.signups;
    Ok(SignupTemplate {
        session: Session::try_borrow_from(state),
        connection,
        signup_enabled,
    }
    .to_response(state))
}

#[derive(Template)]
#[template(path = "signup-result.html")]
struct SignupResultTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn signup_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let new_user: NewUser = serde_urlencoded::from_bytes(&post)?;

    // If the `phone` field is filled out we caught a spammer
    if !new_user.phone.is_empty() {
        // Get client ip address as string
        let addr = match client_addr(state) {
            Some(addr) => format!("{}", addr),
            None => String::from("unavailable"),
        };
        // Log spam attempt
        println!(
            "Caught spam user with id '{}' and client IP '{}'",
            new_user.id, addr,
        );
        return Err(failure::err_msg(
            "You're not supposed to fill out this field",
        ));
    }

    let connection = &DbConnection::from_state(state)?;
    user::create(connection, new_user.clone())?;
    let credentials: Login = new_user.into();

    let session = credentials.login(connection)?.unwrap();
    let mut response = SignupResultTemplate {
        session: Some(&session),
        connection,
    }
    .to_response(state);
    let cookie = session_cookie(state, &session.id);
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.to_string().parse()?);

    Ok(response)
}

#[derive(Template)]
#[template(path = "logout.html")]
struct LogoutTemplate<'a> {
    connection: &'a Connection,
    session: Option<&'a Session>,
}

pub fn logout(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let session = Session::try_borrow_from(state);

    if let Some(session) = session {
        user::logout(connection, &session.id)?;
    }

    let mut response = LogoutTemplate {
        connection,
        session: None,
    }
    .to_response(state);

    // Delete session cookie with Max-Age=0
    let cookie = Cookie::build("session", "")
        .max_age(time::Duration::zero())
        .finish();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.to_string().parse()?);

    Ok(response)
}

#[derive(Template)]
#[template(path = "user.html")]
struct UserTemplate<'a> {
    user: &'a User,
    comments: &'a [CommentTemplate<'a>],
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn view(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let session = Session::try_borrow_from(state);

    let user_id = &UserPath::borrow_from(state).user;
    let user = user::get(connection, user_id)?;
    let comments = comment::by_user(connection, user_id)?;
    let comment_templates = CommentTemplate::from_list(&comments, connection, session, false);

    let template = UserTemplate {
        user: &user,
        comments: &comment_templates,
        session,
        connection,
    };

    Ok(template.to_response(state))
}

#[derive(Template)]
#[template(path = "user-edit.html")]
struct UserProfileTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    user: &'a User,
}

/// Form for editing your account
pub fn edit(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let session = Session::try_borrow_from(state);

    let user_id = &UserPath::borrow_from(state).user;
    let user = user::get(connection, &user_id)?;

    let template = UserProfileTemplate {
        session,
        connection,
        user: &user,
    };
    Ok(template.to_response(state))
}

// TODO: verify permissions are being checked
/// Result for changing profile information
pub fn profile_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let profile: UserProfile = serde_urlencoded::from_bytes(&post)?;
    let connection = &DbConnection::from_state(state)?;
    let user_id = &UserPath::borrow_from(state).user;

    user::edit_profile(connection, user_id, &profile)?;

    let mut response = temp_redirect(state, format!("/user/{}", user_id));
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

/// Result for changing password
pub fn password_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let change: PasswordChange = serde_urlencoded::from_bytes(&post)?;
    let connection = &DbConnection::from_state(state)?;
    let user_id = &UserPath::borrow_from(state).user;

    if !user::change_password(connection, &user_id, &change)? {
        return Err(failure::err_msg("Wrong password"));
    }

    let mut response = temp_redirect(state, format!("/user/{}", user_id));
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

/// Result for deleting your account
pub fn delete_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let deletion: UserDeletion = serde_urlencoded::from_bytes(&post)?;
    let user_id = &UserPath::borrow_from(state).user;

    user::delete(connection, &user_id, &deletion)?;

    let mut response = temp_redirect(state, "/");
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}
