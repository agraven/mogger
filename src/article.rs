use chrono::naive::NaiveDateTime;
use comrak::{markdown_to_html, ComrakOptions};
use diesel::pg::PgConnection as Connection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::Queryable;
use diesel::RunQueryDsl;

use crate::schema::articles;

use crate::user::User;

const PREVIEW_LEN: usize = 500;
const DESCRIPTION_LEN: usize = 160;

#[derive(Debug, Deserialize, Serialize, Queryable, Identifiable)]
pub struct Article {
    /// The article's numeric id
    pub id: i32,
    /// The title of the article
    pub title: String,
    /// The article's author
    pub author: String,
    /// The pretty url of the article
    pub url: String,
    /// The article's content/body
    pub content: String,
    /// The time of publishing
    #[serde(with = "crate::date_format")]
    pub date: NaiveDateTime,
    /// Whether the article has been published
    pub visible: bool,
}

impl Article {
    /// Get the user who submitted this article
    pub fn user(&self, connection: &Connection) -> Result<User, DieselError> {
        crate::schema::users::dsl::users
            .find(&self.author)
            .first(connection)
    }

    /// Return the marked up version of the article's body.
    pub fn formatted(&self) -> String {
        markdown_to_html(&self.content, &ComrakOptions::default())
    }

    /// Get a short slice of the article's contents.
    pub fn description(&self) -> &str {
        let mut end = DESCRIPTION_LEN;
        while !self.content.is_char_boundary(end) {
            end -= 1;
        }
        &self.content[..end]
    }

    /// Used when displaying a preview of the article's contents in a list of articles.
    pub fn preview(&self) -> &str {
        let len = self.content.len();
        if len < PREVIEW_LEN {
            return &self.content[..len];
        }

        // Get a valid index
        let mut end = PREVIEW_LEN;
        while !self.content.is_char_boundary(end) {
            end -= 1;
        }
        // Find whitespace closest to the end
        let end = self.content[..end]
            .rfind(char::is_whitespace)
            .unwrap_or(end);
        &self.content[..end]
    }

    pub fn comment_count(&self, connection: &Connection) -> Result<i64, DieselError> {
        comment_count(connection, self.id)
    }
}

#[derive(Insertable, Deserialize, Serialize)]
#[table_name = "articles"]
pub struct NewArticle {
    pub title: String,
    pub url: String,
    pub content: String,
    pub author: String,
    #[serde(default)]
    pub visible: bool,
}

#[derive(AsChangeset, Deserialize, Serialize)]
#[table_name = "articles"]
pub struct ArticleChanges {
    pub title: String,
    pub url: String,
    pub content: String,
    #[serde(default)]
    pub visible: bool,
}

pub fn id_from_url(connection: &Connection, url: &str) -> Result<i32, DieselError> {
    use crate::schema::articles::dsl;
    let article: Article = dsl::articles.filter(dsl::url.eq(url)).first(connection)?;
    Ok(article.id)
}

pub fn list(connection: &Connection) -> Result<Vec<Article>, DieselError> {
    use crate::schema::articles::dsl::*;

    articles.order(date.desc()).load::<Article>(connection)
}

pub fn view(connection: &Connection, name: &str) -> Result<Article, DieselError> {
    use crate::schema::articles::dsl::*;

    match name.parse::<i32>() {
        Ok(name) => articles.find(name).first(connection),
        Err(_) => articles.filter(url.eq(name)).first(connection),
    }
}

pub fn submit(connection: &Connection, article: &NewArticle) -> Result<usize, DieselError> {
    diesel::insert_into(articles::table)
        .values(article)
        .execute(connection)
}

pub fn edit(
    connection: &Connection,
    id: i32,
    changes: &ArticleChanges,
) -> Result<usize, DieselError> {
    use crate::schema::articles::dsl;

    diesel::update(dsl::articles.find(id))
        .set(changes)
        .execute(connection)
}

pub fn delete(connection: &Connection, name: i32) -> Result<usize, DieselError> {
    use crate::schema::articles::dsl::*;

    diesel::delete(articles.find(&name)).execute(connection)
}

pub fn comment_count(connection: &Connection, id: i32) -> Result<i64, DieselError> {
    use crate::schema::comments::dsl;
    dsl::comments
        .filter(dsl::article.eq(id))
        .count()
        .first(connection)
}

pub fn author(connection: &Connection, id: i32) -> Result<String, DieselError> {
    use crate::schema::articles::dsl;
    dsl::articles.select(dsl::author).find(id).first(connection)
}
