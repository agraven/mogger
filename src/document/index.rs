use askama::Template;
use cookie::Cookie;
use diesel::PgConnection as Connection;
use gotham::{
    helpers::http::response::create_temporary_redirect as temp_redirect,
    state::{FromState, State},
};
use hyper::{header, StatusCode};

use super::{DocumentResult, TemplateExt};
use crate::{
    article::{self, Article, ArticleChanges, NewArticle},
    comment,
    handler::articles::{ArticleIdPath, ArticlePath},
    user::{
        self, Login, NewUser, Permission,
        Permission::{CreateArticle, EditArticle, EditForeignArticle},
        Session,
    },
    DbConnection,
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    articles: Vec<Article>,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn handler(state: &State) -> DocumentResult {
    let connection = &DbConnection::borrow_from(state).lock()?;

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
#[template(path = "article.html", escape = "none")]
pub struct ArticleTemplate<'a> {
    article: Article,
    author_name: String,
    comments: Vec<CommentTemplate<'a>>,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

#[derive(Template)]
#[template(path = "comments.html", escape = "none")]
pub struct CommentTemplate<'a> {
    comment: &'a comment::Comment,
    children: Vec<CommentTemplate<'a>>,
    connection: &'a Connection,
    session: Option<&'a Session>,
    article_id: i32,
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
        article_id: i32,
    ) -> Self {
        CommentTemplate {
            comment: &tree.comment,
            children: tree
                .children
                .iter()
                .map(|child| CommentTemplate::from_node(child, connection, session, article_id))
                .collect(),
            connection,
            session,
            article_id,
        }
    }
}

pub fn article(state: &State) -> DocumentResult {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = &ArticlePath::borrow_from(state).id;
    let session = Session::try_borrow_from(state);

    let article = article::view(connection, &id)?;
    let id = ArticlePath::borrow_from(state).find_id(connection)?;
    let comments = comment::list(connection, id)?;
    let comments_template = comments
        .iter()
        .map(|child| CommentTemplate::from_node(child, connection, session, article.id))
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
    let connection = &DbConnection::borrow_from(state).lock()?;
    Ok(LoginTemplate {
        session: Session::try_borrow_from(state),
        connection,
    }
    .to_response(state))
}

pub fn login_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let connection = &DbConnection::borrow_from(state).lock()?;
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
        let cookie = Cookie::build("session", session.id).finish();
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
    let connection = &DbConnection::borrow_from(state).lock()?;
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
    let connection = &DbConnection::borrow_from(state).lock()?;
    // TODO: check password strength and other input validation
    user::create(connection, new_user.clone())?;
    let credentials: Login = new_user.into();

    let session = credentials.login(connection)?.unwrap();
    let mut response = SignupResultTemplate {
        session: Some(&session),
        connection,
    }
    .to_response(state);
    let cookie = Cookie::build("session", session.id).finish();
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
    let connection = &DbConnection::borrow_from(state).lock()?;
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
#[template(path = "edit.html")]
struct EditTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    article: Option<Article>,
}

pub fn edit(state: &State) -> DocumentResult {
    let connection = &DbConnection::borrow_from(state).lock()?;
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
    let conn = &DbConnection::borrow_from(state).lock()?;

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

        // TODO: url server side format validation
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
    let connection = &DbConnection::borrow_from(state).lock()?;
    Ok(InitSetupTemplate {
        session: Session::try_borrow_from(state),
        connection,
    }
    .to_response(state))
}

pub fn init_setup_post(state: &State, post: Vec<u8>) -> DocumentResult {
    {
        let connection = &DbConnection::borrow_from(state).lock()?;
        if user::count(connection)? > 0 {
            return Err(failure::err_msg("Initial setup already complete"));
        }
    }
    signup_post(state, post)
}
