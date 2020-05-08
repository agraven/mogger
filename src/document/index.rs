use askama::Template;
use cookie::{Cookie, SameSite};
use gotham::{
    helpers::http::response::{create_empty_response, create_temporary_redirect as temp_redirect},
    state::{client_addr, FromState, State},
};
use hyper::{header, StatusCode};

use super::{DocumentResult, TemplateExt};
use crate::{
    article::{self, Article, ArticleChanges, NewArticle},
    comment::{self, Comment},
    db::{Connection, DbConnection},
    handler::{
        articles::{ArticleIdPath, ArticlePath},
        users::UserPath,
    },
    user::{
        self, Login, NewUser, PasswordChange, Permission,
        Permission::{CreateArticle, EditArticle, EditForeignArticle},
        Session, User, UserDeletion, UserProfile,
    },
};

fn session_cookie<'a>(state: &State, id: &str) -> Cookie<'a> {
    let mut cookie = Cookie::build("session", id.to_owned())
        .same_site(SameSite::Strict)
        .http_only(true)
        .finish();
    if http::Uri::borrow_from(state).scheme_part() == Some(&http::uri::Scheme::HTTPS) {
        cookie.set_secure(true);
    }
    cookie
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    articles: Vec<Article>,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn handler(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;

    // If there are no users, redirect to initial setup.
    if user::count(connection)? <= 0 {
        return Ok(temp_redirect(state, "/initial-setup"));
    }

    let articles = article::list(connection)?;

    let session = Session::try_borrow_from(state);

    let template = Index {
        articles,
        session,
        connection,
    };
    let reponse = template.to_response(state);
    Ok(reponse)
}

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn about(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let template = AboutTemplate {
        session: Session::try_borrow_from(state),
        connection,
    };
    Ok(template.to_response(state))
}

#[derive(Template)]
#[template(path = "article.html", escape = "none")]
pub struct ArticleTemplate<'a> {
    article: Article,
    author_name: String,
    comments: Vec<CommentTemplate<'a>>,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

#[derive(Template)]
#[template(path = "comments.html")]
pub struct CommentTemplate<'a> {
    pub comment: &'a comment::Comment,
    pub children: Vec<CommentTemplate<'a>>,
    pub connection: &'a Connection,
    pub session: Option<&'a Session>,
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

impl<'a> CommentTemplate<'a> {
    fn from_node(
        tree: &'a comment::Node,
        connection: &'a Connection,
        session: Option<&'a Session>,
    ) -> Self {
        CommentTemplate {
            comment: &tree.comment,
            children: tree
                .children
                .iter()
                .map(|child| CommentTemplate::from_node(child, connection, session))
                .collect(),
            connection,
            session,
        }
    }

    fn from_list(
        list: &'a [Comment],
        connection: &'a Connection,
        session: Option<&'a Session>,
    ) -> Vec<Self> {
        list.iter()
            .map(|comment| CommentTemplate {
                comment,
                children: Vec::new(),
                connection,
                session,
            })
            .collect()
    }
}

pub fn article(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let id = &ArticlePath::borrow_from(state).id;
    let session = Session::try_borrow_from(state);

    let article = article::view(connection, &id)?;
    // Return a 404 if the user isn't allowed to view the article
    if !article.viewable(session, connection)? {
        return Ok(create_empty_response(state, StatusCode::NOT_FOUND));
    }

    let comments = comment::list(connection, article.id)?;
    let comments_template = comments
        .iter()
        .map(|child| CommentTemplate::from_node(child, connection, session))
        .collect();
    let author = article.user(connection)?;
    let template = ArticleTemplate {
        article,
        author_name: author.name,
        comments: comments_template,
        session,
        connection,
    };
    let response = template.to_response(state);
    Ok(response)
}

pub fn login(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    Ok(LoginTemplate {
        session: Session::try_borrow_from(state),
        connection,
    }
    .to_response(state))
}

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
        // TODO: Add security settings for cookie without breaking debugging.
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
}

pub fn signup(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    Ok(SignupTemplate {
        session: Session::try_borrow_from(state),
        connection,
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

pub fn user(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let session = Session::try_borrow_from(state);

    let user_id = &UserPath::borrow_from(state).user;
    let user = user::get(connection, user_id)?;
    let comments = comment::by_user(connection, user_id)?;
    let comment_templates = CommentTemplate::from_list(&comments, connection, session);

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

pub fn user_edit(state: &State) -> DocumentResult {
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

pub fn user_profile_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let profile: UserProfile = serde_urlencoded::from_bytes(&post)?;
    let connection = &DbConnection::from_state(state)?;
    let user_id = &UserPath::borrow_from(state).user;

    user::edit_profile(connection, user_id, &profile)?;

    let mut response = temp_redirect(state, format!("/user/{}", user_id));
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

pub fn user_password_post(state: &State, post: Vec<u8>) -> DocumentResult {
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

pub fn user_delete_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let deletion: UserDeletion = serde_urlencoded::from_bytes(&post)?;
    let user_id = &UserPath::borrow_from(state).user;

    user::delete(connection, &user_id, &deletion)?;

    let mut response = temp_redirect(state, "/");
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

#[derive(Template)]
#[template(path = "edit.html")]
struct EditTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    article: Option<Article>,
}

pub fn edit(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let article = match ArticleIdPath::try_borrow_from(state) {
        Some(path) => Some(article::view(connection, &path.id.to_string())?),
        None => None,
    };
    Ok(EditTemplate {
        session: Session::try_borrow_from(state),
        connection,
        article,
    }
    .to_response(state))
}

pub fn edit_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let session = Session::try_borrow_from(state);
    let conn = &DbConnection::from_state(state)?;

    let url = if let Some(path) = ArticleIdPath::try_borrow_from(state) {
        let changes: ArticleChanges = serde_urlencoded::from_bytes(&post)?;

        // Check permissions
        match session {
            Some(s)
                if s.allowed(EditForeignArticle, conn)?
                    || s.allowed(EditArticle, conn)?
                        && s.user == article::author(conn, path.id)? =>
            {
                ()
            }
            _ => return Err(failure::err_msg("Permission denied")),
        };

        article::edit(conn, path.id, &changes)?;
        changes.url
    } else {
        let new_article: NewArticle = serde_urlencoded::from_bytes(&post)?;

        match session {
            Some(session) if session.allowed(CreateArticle, conn)? => (),
            _ => return Err(failure::err_msg("Permission denied")),
        }

        article::submit(conn, &new_article)?;
        new_article.url
    };
    // Redirect to page for the new article
    let mut response = temp_redirect(state, format!("/article/{}", url));
    // Force method to be GET
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

#[derive(Template)]
#[template(path = "initial-setup.html")]
pub struct InitSetupTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn init_setup(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    Ok(InitSetupTemplate {
        session: Session::try_borrow_from(state),
        connection,
    }
    .to_response(state))
}

pub fn init_setup_post(state: &State, post: Vec<u8>) -> DocumentResult {
    {
        // Have this in a separate scope so the connection lock gets dropped
        let connection = &DbConnection::from_state(state)?;
        if user::count(connection)? > 0 {
            return Err(failure::err_msg("Initial setup already complete"));
        }
    }
    signup_post(state, post)
}
