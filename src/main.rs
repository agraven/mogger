//! A simple blogging engine.
//!
//! It has the following address scheme:
//! * `/api` - JSON interface
//!     * `/article` - Article handling
//!         * `/list` - List all articles
//!         * `/view/<url|id>` - Gets the article with the specified `url` or `id`
//!         * `/submit` - Submit an article
//!         * `/edit/<url|id>` - Edit the article with the specified `url` or `id`
//!     * `/comments` - Comment handling
//!         * `/list/<url|id>` - Gets the comments for the specified article
//!         * `/view/<id>` - Gets the comment with `id`
//!         * `/submit` - Submits a comment
//!         * `/edit/<id>` - Edits the comment with `id`
//!         * `/delete/<id>` - Deletes the selected comment (i.e. marks it as hidden)
//!         * `/purge/<id>` - Purges the selected comment (i.e. removes from the database)
//!     * `/users` - User handling
//!         * `/create` - Create a user
//! * `/feed.rss` - RSS feed
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serde;

pub mod article;
pub mod comment;
pub mod date_format;
pub mod db;
pub mod document;
pub mod handler;
pub mod schema;
pub mod user;

pub use diesel::pg::PgConnection as Connection;
use gotham::{
    middleware::cookie::CookieParser,
    middleware::state::StateMiddleware,
    pipeline::new_pipeline,
    pipeline::single::single_pipeline,
    //pipeline::single_middleware,
    router::builder::{build_router, DefineSingleRoute, DrawRoutes},
    router::response::extender::ResponseExtender,
    router::Router,
    state::State,
};
use gotham_derive::StateData;
use http::status::StatusCode;
use hyper::{Body, Response};

use std::sync::{Arc, Mutex};

use crate::user::SessionMiddleware;

/// Response extender for 404 errors
pub struct NotFound;

impl ResponseExtender<Body> for NotFound {
    fn extend(&self, _state: &mut State, res: &mut Response<Body>) {
        let body = res.body_mut();
        *body = "404 File not found".into();
    }
}

#[derive(Clone, StateData)]
pub struct DbConnection {
    connection: Arc<Mutex<Connection>>,
}

impl DbConnection {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(db::connect())),
        }
    }

    pub fn get(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    pub fn lock(&self) -> Result<std::sync::MutexGuard<Connection>, failure::Error> {
        match self.connection.lock() {
            Ok(lock) => Ok(lock),
            Err(_) => Err(failure::err_msg("failed to get lock")),
        }
    }
}

fn router() -> Router {
    // The directory static assets are served from. Is:
    // STATIC_DIR environment varible if defined, otherwise
    // STATIC_DIR compile-time environment variable if defined, otherwise
    // local directory 'static'
    let assets_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| option_env!("STATIC_DIR").unwrap_or("static").to_owned());

    // Set up shared state
    let connection = DbConnection::new();
    let state_mw = StateMiddleware::new(connection);
    // Build pipeline
    let (chain, pipelines) = single_pipeline(
        new_pipeline()
            .add(state_mw)
            .add(CookieParser)
            .add(SessionMiddleware)
            .build(),
    );

    build_router(chain, pipelines, |route| {
        use crate::handler::articles;
        route.get("/").to(handler!(document::index::handler));

        route
            .get("/article/:id")
            .with_path_extractor::<articles::ArticlePath>()
            .to(handler!(document::index::article));

        route.get("/login").to(handler!(document::index::login));
        route
            .post("/login")
            .to(body_handler!(document::index::login_post));

        route.get("/signup").to(handler!(document::index::signup));
        route
            .post("/signup")
            .to(body_handler!(document::index::signup_post));

        route.get("/edit").to(handler!(document::index::edit));
        route
            .post("/edit")
            .to(body_handler!(document::index::edit_post));

        route.scope("/api", |route| {
            route.scope("/articles", |route| {
                route.get("/list").to(handler!(articles::list));
                route
                    .get("/view/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(handler!(articles::view));
                route.post("/submit").to(body_handler!(articles::submit));
                route
                    .post("/edit/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(body_handler!(articles::edit));
            });

            route.scope("/comments", |route| {
                use crate::handler::comments;

                route
                    .get("/list/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(handler!(comments::list));

                route
                    .get("/view/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .with_query_string_extractor::<comments::Context>()
                    .to(handler!(comments::view));

                route.post("/submit").to(body_handler!(comments::submit));

                route
                    .post("/edit/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(body_handler!(comments::edit));

                route
                    .get("/delete/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::delete));

                route
                    .get("/purge/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::purge))
            });

            route.scope("/users", |route| {
                use crate::handler::users;

                route.post("/create").to(body_handler!(users::create));
                route.post("/login").to(body_handler!(users::login));
            });
        });

        route.get("/file/*").to_dir(assets_dir);

        route.get("/feed.rss").to(handler!(handler::rss::rss));

        // Error responders
        route.add_response_extender(StatusCode::NOT_FOUND, NotFound);
    })
}

fn main() {
    let address = "127.0.0.1:6096";
    println!("Running at {}", address);
    gotham::start(address, router())
}
