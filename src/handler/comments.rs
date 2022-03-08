use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    hyper::{Body, Response, StatusCode},
    state::{FromState, State, StateData},
    router::response::StaticResponseExtender,
    mime::{APPLICATION_JSON as JSON, TEXT_HTML},
};

use crate::{
    comment,
    comment::{CommentChanges, NewComment},
    config::Settings,
    document::TemplateExt,
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
    let id = ArticlePath::borrow_from(state).find_id(connection)?;

    let comments = comment::list(connection, id)?;
    let content = serde_json::to_string(&comments)?;
    Ok(create_response(state, StatusCode::OK, JSON, content))
}

pub fn view(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let query = Context::borrow_from(state);
    let context = query.context.unwrap_or(0);
    let id = CommentPath::borrow_from(state).id;

    let comment = comment::view(connection, id, context)?;
    let content = serde_json::to_string(&comment)?;
    Ok(create_response(state, StatusCode::OK, JSON, content))
}

pub fn single(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    let comment = comment::view_single(connection, id)?;
    let content = serde_json::to_string(&comment)?;
    Ok(create_response(state, StatusCode::OK, JSON, content))
}

pub fn render_content(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    if let Some(comment) = comment::view_single(connection, id)? {
        Ok(create_response(
            state,
            StatusCode::OK,
            TEXT_HTML,
            comment.formatted(),
        ))
    } else {
        Ok(create_empty_response(state, StatusCode::NOT_FOUND))
    }
}

pub fn render(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    if let Some(mut comment) = comment::view_single(connection, id)? {
        comment.visible = true;
        let session = Session::try_borrow_from(state);
        let can_comment = session.is_some();
        let template = crate::document::article::CommentTemplate {
            comment: &comment,
            children: Vec::new(),
            connection,
            session,
            can_comment,
        };
        Ok(template.to_response(state))
    } else {
        Ok(create_empty_response(state, StatusCode::NOT_FOUND))
    }
}

pub fn submit(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let session = Session::try_borrow_from(state);
    let settings = Settings::borrow_from(state);
    if session.is_none() && !settings.features.guest_comments {
        return Err(failure::err_msg("Permission denied"));
    }
    let connection = &DbConnection::borrow_from(state).lock()?;

    let mut new: NewComment = serde_json::from_slice(&post)?;
    // Make guest comments invisible by default
    if new.author.is_some() {
        new.visible = true;
    }
    // Ensure comment is submitted as own user
    if new.author.as_ref() != session.as_ref().map(|s| &s.user) {
        return Err(failure::err_msg("Permission denied"));
    }

    let submitted = comment::submit(connection, new)?;
    let content = serde_json::to_string(&submitted)?;
    Ok(create_response(state, StatusCode::OK, JSON, content))
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
        }
        _ => return Err(failure::err_msg("Permission denied")),
    };

    let changes: CommentChanges = serde_json::from_slice(&post)?;

    comment::edit(connection, id, changes)?;
    Ok(create_empty_response(state, StatusCode::OK))
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
        }
        _ => return Err(failure::err_msg("Permission denied")),
    };

    comment::delete(conn, id)?;
    Ok(create_empty_response(state, StatusCode::OK))
}

pub fn restore(state: &State) -> Result<Response<Body>, failure::Error> {
    let conn = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    match Session::try_borrow_from(state) {
        Some(session)
            if session.allowed(DeleteForeignComment, conn)?
                || session.allowed(DeleteComment, conn)?
                    && comment::author(conn, id)?.as_ref() == Some(&session.user) =>
        {
        }
        _ => return Err(failure::err_msg("Permission denied")),
    };

    comment::restore(conn, id)?;
    Ok(create_empty_response(state, StatusCode::OK))
}

pub fn purge(state: &State) -> Result<Response<Body>, failure::Error> {
    let conn = &DbConnection::borrow_from(state).lock()?;
    let id = CommentPath::borrow_from(state).id;

    match Session::try_borrow_from(state) {
        Some(session) if session.allowed(DeleteForeignComment, conn)? => (),
        _ => return Err(failure::err_msg("Permission denied")),
    };

    comment::purge(conn, id)?;
    Ok(create_empty_response(state, StatusCode::OK))
}
