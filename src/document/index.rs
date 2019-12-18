use askama::Template;
use cookie::Cookie;
use failure::err_msg;
use gotham::state::{FromState, State};
use hyper::{header, Body, Response};

use super::TemplateExt;
use crate::{
    article,
    article::Article,
    comment,
    handler::articles::ArticlePath,
    user::{Login, Session},
    DbConnection,
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    articles: Vec<Article>,
    session: Option<&'a Session>,
}

pub fn handler(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let articles = article::list(connection)?;

    let session = Session::try_borrow_from(state);

    let template = Index { articles, session };
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
