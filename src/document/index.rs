//! Module for the index, static info pages and initial signup

use askama::Template;
use gotham::{
    helpers::http::response::create_temporary_redirect as temp_redirect,
    state::{FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};

use super::{DocumentResult, TemplateExt};
use crate::{
    article::{self, Article},
    db::{Connection, DbConnection},
    user::{self, Permission, Session},
};

/// Page number in a paginated document
#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct Page {
    page: i64,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    articles: Vec<Article>,
    page: i64,
    session: Option<&'a Session>,
    connection: &'a Connection,
}

/// Index. Shows a paginated list of published articles.
pub fn index(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;

    // If there are no users, redirect to initial setup.
    if user::count(connection)? <= 0 {
        return Ok(temp_redirect(state, "/initial-setup"));
    }

    let page = match Page::try_borrow_from(state) {
        Some(page) => page.page,
        None => 1,
    };
    let articles = article::page(connection, page)?;

    let session = Session::try_borrow_from(state);

    let template = Index {
        articles,
        page,
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

/// About page
pub fn about(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let template = AboutTemplate {
        session: Session::try_borrow_from(state),
        connection,
    };
    Ok(template.to_response(state))
}
#[derive(Template)]
#[template(path = "initial-setup.html")]
pub struct InitSetupTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
}

/// Initial setup (i.e. create admin user) form
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
    crate::document::user::signup_post(state, post)
}
