use diesel::result::Error as DieselError;
use diesel::PgConnection as Connection;
use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    state::{FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};
use http::{Response, StatusCode};
use hyper::Body;
use mime::APPLICATION_JSON as JSON;

use crate::{
    article::{self, ArticleChanges, NewArticle},
    user::{Permission, Session},
    DbConnection,
};

#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct ArticlePath {
    pub id: String,
}

/// An article id or url
impl ArticlePath {
    pub fn find_id(&self, connection: &Connection) -> Result<i32, DieselError> {
        match self.id.parse::<i32>() {
            Ok(id) => Ok(id),
            Err(_) => article::id_from_url(connection, &self.id),
        }
    }
}

#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct ArticleIdPath {
    pub id: i32,
}

pub fn list(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;

    let articles = article::list(&connection)?;
    let content = serde_json::to_string(&articles)?;
    let response = create_response(&state, StatusCode::OK, JSON, content);
    Ok(response)
}

pub fn view(state: &State) -> Result<Response<Body>, failure::Error> {
    let id = &ArticlePath::borrow_from(&state).id;

    let connection = &DbConnection::borrow_from(&state).lock()?;

    let article = article::view(connection, id)?;
    let content = serde_json::to_string(&article)?;
    let response = create_response(&state, StatusCode::OK, JSON, content);
    Ok(response)
}

pub fn submit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(&state).lock()?;

    // Check for CreateArticle permission
    match Session::try_borrow_from(state) {
        Some(session) if session.allowed(Permission::CreateArticle, connection)? => (),
        _ => return Err(failure::err_msg("Permission denied")),
    }

    let new: NewArticle = serde_json::from_slice(&post)?;

    article::submit(connection, &new)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn edit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(&state).lock()?;
    let id = ArticlePath::borrow_from(&state).find_id(connection)?;

    // Check for EditArticle or EditForeignArticle permission.
    match Session::try_borrow_from(state) {
        Some(session)
            if session.allowed(Permission::EditForeignArticle, connection)?
                || session.allowed(Permission::EditArticle, connection)?
                    && article::author(connection, id)? == session.user =>
        {
            ()
        }
        _ => return Err(failure::err_msg("Permission denied")),
    }

    let changes: ArticleChanges = serde_json::from_slice(&post)?;

    article::edit(&connection, id, &changes)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn delete(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(&state).lock()?;
    let id = ArticlePath::borrow_from(&state).find_id(connection)?;

    match Session::try_borrow_from(state) {
        Some(session)
            if session.allowed(Permission::DeleteForeignArticle, connection)?
                || session.allowed(Permission::DeleteArticle, connection)?
                    && article::author(connection, id)? == session.user =>
        {
            ()
        }
        _ => return Err(failure::err_msg("Permission denied")),
    }

    article::delete(connection, id)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}
