use failure::err_msg;
use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    state::{FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};
use http::{Response, StatusCode};
use hyper::Body;
use mime::APPLICATION_JSON as JSON;

use crate::{
    comment,
    comment::{CommentChanges, NewComment},
    handler::articles::ArticlePath,
    DbConnection,
};

#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct CommentPath {
    id: i32,
}

#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct Context {
    context: Option<u32>,
}

pub fn list(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let id = ArticlePath::borrow_from(&state).find_id(connection)?;

    let comments = comment::list(connection, id)?;
    let content = serde_json::to_string(&comments)?;
    Ok(create_response(&state, StatusCode::OK, JSON, content))
}

pub fn view(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let query = Context::borrow_from(&state);
    let context = query.context.unwrap_or(0);
    let id = CommentPath::borrow_from(&state).id;

    let comment = comment::view(connection, id, context)?;
    let content = serde_json::to_string(&comment)?;
    Ok(create_response(&state, StatusCode::OK, JSON, content))
}

pub fn submit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;

    let new: NewComment = serde_json::from_slice(&post)?;

    comment::submit(connection, new)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn edit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let id = CommentPath::borrow_from(state).id;

    let changes: CommentChanges = serde_json::from_slice(&post)?;

    comment::edit(connection, id, changes)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn delete(state: &State) -> Result<Response<Body>, failure::Error> {
    let arc = DbConnection::borrow_from(&state).get();
    let connection = &arc.lock().or(Err(err_msg("async error")))?;
    let id = CommentPath::borrow_from(state).id;

    comment::delete(connection, id)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}
