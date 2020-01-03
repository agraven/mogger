use diesel::result::Error as DieselError;
use diesel::PgConnection as Connection;
use failure::err_msg;
use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    state::{FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};
use http::{Response, StatusCode};
use hyper::Body;
use mime::APPLICATION_JSON as JSON;

use crate::article::{self, NewArticle};
use crate::DbConnection;

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

pub fn list(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(&state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    let articles = article::list(&connection)?;
    let content = serde_json::to_string(&articles)?;
    let response = create_response(&state, StatusCode::OK, JSON, content);
    Ok(response)
}

pub fn view(state: &State) -> Result<Response<Body>, failure::Error> {
    let id = &ArticlePath::borrow_from(&state).id;
    let arc = DbConnection::borrow_from(&state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    let article = article::view(connection, id)?;
    let content = serde_json::to_string(&article)?;
    let response = create_response(&state, StatusCode::OK, JSON, content);
    Ok(response)
}

pub fn submit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    let new: NewArticle = serde_json::from_slice(&post)?;

    article::submit(connection, &new)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn edit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(&state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    let id = ArticlePath::borrow_from(&state).find_id(connection)?;
    let changes: NewArticle = serde_json::from_slice(&post)?;

    article::edit(&connection, id, changes)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn delete(state: &State) -> Result<Response<Body>, failure::Error> {
    let id = &ArticlePath::borrow_from(&state).id;
    let arc = DbConnection::borrow_from(&state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    article::delete(connection, id)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}
