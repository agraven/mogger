use askama::Template;
use failure::err_msg;
use gotham::{
    handler::IntoResponse,
    state::{FromState, State},
};
use hyper::{Body, Response};

use crate::{article, article::Article, comment, handler::articles::ArticlePath, DbConnection};

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    articles: Vec<Article>,
}

pub fn handler(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let articles = article::list(connection)?;

    let template = Index { articles: articles };
    let reponse = template.into_response(state);
    Ok(reponse)
}

#[derive(Template)]
#[template(path = "article.html")]
pub struct ArticleTemplate<'a> {
    article: Article,
    author_name: String,
    comments: Vec<CommentTemplate<'a>>,
}

#[derive(Template)]
#[template(path = "comments.html")]
pub struct CommentTemplate<'a> {
    comment: &'a comment::Comment,
    children: Vec<CommentTemplate<'a>>,
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
    let template = ArticleTemplate {
        article,
        author_name: "TODO".to_owned(),
        comments: comments_template,
    };
    let response = template.into_response(state);
    Ok(response)
}
