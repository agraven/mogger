//! Templates and request handlers for endpoints related to displaying articles
use askama::Template;
use gotham::{
    helpers::http::response::{create_empty_response, create_temporary_redirect as temp_redirect},
    hyper::StatusCode,
    state::{FromState, State},
};

use crate::{
    article::{self, Article, ArticleChanges, NewArticle},
    comment::{self, Comment},
    config::Settings,
    db::{Connection, DbConnection},
    document::{DocumentResult, TemplateExt},
    handler::articles::{ArticleIdPath, ArticlePath},
    user::{
        Permission,
        Permission::{CreateArticle, EditArticle, EditForeignArticle},
        Session,
    },
};

#[derive(Template)]
#[template(path = "article.html", escape = "none")]
pub struct ArticleTemplate<'a> {
    article: Article,
    author_name: String,
    comments: Vec<CommentTemplate<'a>>,
    session: Option<&'a Session>,
    connection: &'a Connection,
    can_comment: bool,
}

#[derive(Template)]
#[template(path = "comments.html")]
pub struct CommentTemplate<'a> {
    pub comment: &'a Comment,
    pub children: Vec<CommentTemplate<'a>>,
    pub connection: &'a Connection,
    pub session: Option<&'a Session>,
    pub can_comment: bool,
}

impl<'a> CommentTemplate<'a> {
    pub fn from_node(
        tree: &'a comment::Node,
        connection: &'a Connection,
        session: Option<&'a Session>,
        can_comment: bool,
    ) -> Self {
        CommentTemplate {
            comment: &tree.comment,
            children: tree
                .children
                .iter()
                .map(|child| CommentTemplate::from_node(child, connection, session, can_comment))
                .collect(),
            connection,
            session,
            can_comment,
        }
    }

    pub fn from_list(
        list: &'a [Comment],
        connection: &'a Connection,
        session: Option<&'a Session>,
        can_comment: bool,
    ) -> Vec<Self> {
        list.iter()
            .map(|comment| CommentTemplate {
                comment,
                children: Vec::new(),
                connection,
                session,
                can_comment,
            })
            .collect()
    }
}

#[derive(Template)]
#[template(path = "edit.html")]
struct EditTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    article: Option<Article>,
}

/// Display an article
pub fn view(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let id = &ArticlePath::borrow_from(state).id;
    let session = Session::try_borrow_from(state);
    let can_comment = Settings::borrow_from(state).features.guest_comments || session.is_some();

    let article = article::view(connection, id)?;
    // Return a 404 if the user isn't allowed to view the article
    if !article.viewable(session, connection)? {
        return Ok(create_empty_response(state, StatusCode::NOT_FOUND));
    }

    let comments = comment::list(connection, article.id)?;
    let comments_template = comments
        .iter()
        .map(|child| CommentTemplate::from_node(child, connection, session, can_comment))
        .collect();
    let author = article.user(connection)?;
    // true if logged in or guest comments permitted
    let template = ArticleTemplate {
        article,
        author_name: author.name,
        comments: comments_template,
        session,
        connection,
        can_comment,
    };
    let response = template.to_response(state);
    Ok(response)
}

pub fn edit(state: &State) -> DocumentResult {
    let connection = &DbConnection::from_state(state)?;
    let article = match ArticleIdPath::try_borrow_from(state) {
        Some(path) => Some(article::view(connection, &path.id.to_string())?),
        None => None,
    };
    Ok(EditTemplate {
        session: Session::try_borrow_from(state),
        connection,
        article,
    }
    .to_response(state))
}

pub fn edit_post(state: &State, post: Vec<u8>) -> DocumentResult {
    let session = Session::try_borrow_from(state);
    let conn = &DbConnection::from_state(state)?;

    let url = if let Some(path) = ArticleIdPath::try_borrow_from(state) {
        let changes: ArticleChanges = serde_urlencoded::from_bytes(&post)?;

        // Check permissions
        match session {
            Some(s)
                if s.allowed(EditForeignArticle, conn)?
                    || s.allowed(EditArticle, conn)?
                        && s.user == article::author(conn, path.id)? =>
            {
            }
            _ => return Err(failure::err_msg("Permission denied")),
        };

        article::edit(conn, path.id, &changes)?;
        changes.url
    } else {
        let new_article: NewArticle = serde_urlencoded::from_bytes(&post)?;

        match session {
            Some(session) if session.allowed(CreateArticle, conn)? => (),
            _ => return Err(failure::err_msg("Permission denied")),
        }

        article::submit(conn, &new_article)?;
        new_article.url
    };
    // Redirect to page for the new article
    let mut response = temp_redirect(state, format!("/article/{}", url));
    // Force method to be GET
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}
