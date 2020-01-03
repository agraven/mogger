use askama::Template;
use cookie::Cookie;
use diesel::PgConnection as Connection;
use failure::err_msg;
use gotham::{
    helpers::http::response::create_temporary_redirect as temp_redirect,
    state::{FromState, State},
};
use hyper::{header, Body, Response, StatusCode};

use super::TemplateExt;
use crate::{
    article::{self, Article, NewArticle},
    comment,
    handler::articles::ArticlePath,
    user::{self, Login, NewUser, Session},
    DbConnection,
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    articles: Vec<Article>,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

pub fn handler(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
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
}

#[derive(Template)]
#[template(path = "comments.html", escape = "none")]
pub struct CommentTemplate<'a> {
    comment: &'a comment::Comment,
    children: Vec<CommentTemplate<'a>>,
}

#[derive(Template, Clone)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    session: Option<&'a Session>,
}

#[derive(Template, Clone)]
#[template(path = "login-result.html")]
pub struct LoginResultTemplate<'a> {
    session: Option<&'a Session>,
}

impl<'a> From<&'a comment::Node> for CommentTemplate<'a> {
    fn from(tree: &'a comment::Node) -> Self {
        CommentTemplate {
            comment: &tree.comment,
            children: tree.children.iter().map(CommentTemplate::from).collect(),
        }
    }
}

pub fn article(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let id = &ArticlePath::borrow_from(state).id;

    let article = article::view(connection, &id)?;
    let id = ArticlePath::borrow_from(state).find_id(connection)?;
    let comments = comment::list(connection, id)?;
    let comments_template = comments.iter().map(CommentTemplate::from).collect();
    let author = article.user(connection)?;
    let template = ArticleTemplate {
        article,
        author_name: author.name,
        comments: comments_template,
        session: Session::try_borrow_from(state),
    };
    let response = template.to_response(state);
    Ok(response)
}

pub fn login(state: &State) -> Result<Response<Body>, failure::Error> {
    Ok(LoginTemplate {
        session: Session::try_borrow_from(state),
    }
    .to_response(state))
}

pub fn login_post(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let credentials: Login = serde_urlencoded::from_bytes(&post)?;
    let new_session = credentials.login(connection)?;

    let mut response = LoginResultTemplate {
        session: new_session.as_ref(),
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
}

pub fn signup(state: &State) -> Result<Response<Body>, failure::Error> {
    Ok(SignupTemplate {
        session: Session::try_borrow_from(state),
    }
    .to_response(state))
}

#[derive(Template)]
#[template(path = "signup-result.html")]
struct SignupResultTemplate<'a> {
    session: Option<&'a Session>,
}

pub fn signup_post(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let new_user: NewUser = serde_urlencoded::from_bytes(&post)?;
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    // TODO: check password strength and other input validation
    user::create(connection, new_user.clone())?;
    let credentials: Login = new_user.into();

    let session = credentials.login(connection)?.unwrap();
    let mut response = SignupResultTemplate {
        session: Some(&session),
    }
    .to_response(state);
    let cookie = Cookie::build("session", session.id).finish();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.to_string().parse()?);

    Ok(response)
}

#[derive(Template)]
#[template(path = "edit.html")]
struct EditTemplate<'a> {
    session: Option<&'a Session>,
}

pub fn edit(state: &State) -> Result<Response<Body>, failure::Error> {
    Ok(EditTemplate {
        session: Session::try_borrow_from(state),
    }
    .to_response(state))
}

pub fn edit_post(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let new_article: NewArticle = serde_urlencoded::from_bytes(&post)?;
    let session = Session::try_borrow_from(state);
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().map_err(|_| err_msg("async error"))?;

    // validate submitted username
    match session {
        Some(ref session) if session.user == new_article.author => (),
        _ => return Err(failure::err_msg("Wrong user")),
    }
    // TODO: permission check
    // TODO: url validation
    article::submit(connection, &new_article)?;
    let mut response = temp_redirect(state, format!("/article/{}", new_article.url));
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}
