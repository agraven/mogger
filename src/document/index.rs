use askama::Template;
use failure::err_msg;
use gotham::{
    handler::IntoResponse,
    state::{FromState, State},
};
use hyper::{Body, Response};

use crate::{article, article::Article, DbConnection};

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
