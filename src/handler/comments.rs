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
    user::{
        Permission::{DeleteComment, DeleteForeignComment, EditComment, EditForeignComment},
        Session,
    },
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
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = ArticlePath::borrow_from(&state).find_id(connection)?;

    let comments = comment::list(connection, id)?;
    let content = serde_json::to_string(&comments)?;
    Ok(create_response(&state, StatusCode::OK, JSON, content))
}

pub fn view(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let query = Context::borrow_from(&state);
    let context = query.context.unwrap_or(0);
    let id = CommentPath::borrow_from(&state).id;

    let comment = comment::view(connection, id, context)?;
    let content = serde_json::to_string(&comment)?;
    Ok(create_response(&state, StatusCode::OK, JSON, content))
}

pub fn single(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(&state).id;

    let comment = comment::view_single(connection, id)?;
    let content = serde_json::to_string(&comment)?;
    Ok(create_response(&state, StatusCode::OK, JSON, content))
}

pub fn render(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(&state).id;

    if let Some(comment) = comment::view_single(connection, id)? {
        Ok(create_response(
            &state,
            StatusCode::OK,
            mime::TEXT_HTML,
            comment.formatted(),
        ))
    } else {
        Ok(create_response(
            &state,
            StatusCode::NOT_FOUND,
            mime::TEXT_PLAIN,
            "Not found",
        ))
    }
}

pub fn submit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;

    let new: NewComment = serde_json::from_slice(&post)?;

    comment::submit(connection, new)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn edit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    match Session::try_borrow_from(state) {
        Some(session)
            if session.allowed(EditForeignComment, connection)?
                || session.allowed(EditComment, connection)?
                    && comment::author(connection, id)?.as_ref() == Some(&session.user) =>
        {
            ()
        }
        _ => return Err(failure::err_msg("Permission denied")),
    };

    let changes: CommentChanges = serde_json::from_slice(&post)?;

    comment::edit(connection, id, changes)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn delete(state: &State) -> Result<Response<Body>, failure::Error> {
    let conn = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    match Session::try_borrow_from(state) {
        Some(session)
            if session.allowed(DeleteForeignComment, conn)?
                || session.allowed(DeleteComment, conn)?
                    && comment::author(conn, id)?.as_ref() == Some(&session.user) =>
        {
            ()
        }
        _ => return Err(failure::err_msg("Permission denied")),
    };

    // FIXME
    // Check for same user
    /*let comment = comment::view_single(connection, id)?;
    if comment.and_then(|c| c.author) == session.map(|s| s.user.clone()) {
        return Err(err_msg("Unauthorized"));
    }*/

    comment::delete(conn, id)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}

pub fn purge(state: &State) -> Result<Response<Body>, failure::Error> {
    let conn = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    match Session::try_borrow_from(state) {
        Some(session) if session.allowed(DeleteForeignComment, conn)? => (),
        _ => return Err(failure::err_msg("Permission denied")),
    };

    comment::purge(conn, id)?;
    Ok(create_empty_response(&state, StatusCode::OK))
}
