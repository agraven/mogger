use chrono::NaiveDateTime;
use comrak::markdown_to_html;
use diesel::{pg::PgConnection as Connection, prelude::*, result::Error as DieselError, Queryable};

use crate::{
    schema::comments,
    user::{self, Permission, Session},
};

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Identifiable)]
pub struct Comment {
    /// The unique id of this comment
    pub id: i32,
    /// The id of its parent, if any
    pub parent: Option<i32>,
    /// The id of the article this comment belongs to
    pub article: i32,
    /// The user who submitted the comment
    pub author: Option<String>,
    /// The name to display for guest comments
    pub name: Option<String>,
    /// The comment's content
    pub content: String,
    /// The time of the comment's submission
    #[serde(with = "crate::date_format")]
    pub date: NaiveDateTime,
    /// Whether to display the comment
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "comments"]
pub struct NewComment {
    pub parent: Option<i32>,
    pub article: i32,
    pub author: Option<String>,
    pub name: Option<String>,
    pub content: String,
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, AsChangeset)]
#[table_name = "comments"]
pub struct CommentChanges {
    pub name: Option<String>,
    pub content: String,
    pub visible: bool,
}

/// A tree of comments
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub comment: Comment,
    pub children: Vec<Node>,
}

impl Comment {
    pub fn viewable(
        &self,
        session: Option<&Session>,
        conn: &Connection,
    ) -> Result<bool, DieselError> {
        if self.visible {
            Ok(true)
        } else {
            self.editable(session, conn)
        }
    }

    pub fn editable(
        &self,
        session: Option<&Session>,
        conn: &Connection,
    ) -> Result<bool, DieselError> {
        if let Some(session) = session {
            Ok(session.allowed(Permission::EditForeignComment, conn)?
                || session.allowed(Permission::EditComment, conn)?
                    && self
                        .author
                        .as_ref()
                        .map(|a| a == &session.user)
                        .unwrap_or(false))
        } else {
            Ok(false)
        }
    }

    pub fn formatted(&self) -> String {
        markdown_to_html(&self.content, &crate::COMRAK_OPTS)
    }

    pub fn author(&self, connection: &Connection) -> Result<String, failure::Error> {
        if let Some(name) = self.name.as_ref() {
            return Ok(name.to_owned());
        }
        let user = user::get(connection, &self.author.as_ref().unwrap())?;
        Ok(user.name)
    }
}

impl Node {
    pub fn new(comment: Comment) -> Node {
        Node {
            comment,
            children: Vec::new(),
        }
    }

    /// Get children from comment list and recursively populate them
    pub fn populate(&mut self, list: &[Comment]) {
        // Populate list of children with comments with matching parent
        self.children = list
            .iter()
            .filter(|comment| comment.parent == Some(self.comment.id))
            .cloned()
            .map(Node::new)
            .collect();

        // Recurse on each child.
        for child in self.children.iter_mut() {
            child.populate(list);
        }
    }
}

/// Get a linear list of an articles comments
pub fn list_flat(connection: &Connection, article: i32) -> Result<Vec<Comment>, DieselError> {
    use crate::schema::comments::dsl;

    dsl::comments
        .filter(dsl::article.eq(article))
        .load::<Comment>(connection)
}

/// Get the comments of an article as a tree structure
pub fn list(connection: &Connection, article: i32) -> Result<Vec<Node>, DieselError> {
    let list = list_flat(connection, article)?;

    // Make a vec of root level (i.e. parent is none) comments.
    let mut tree: Vec<Node> = list
        .iter()
        .filter(|comment| comment.parent.is_none())
        .cloned()
        .map(Node::new)
        .collect();

    // Populate the root level comments
    for node in tree.iter_mut() {
        node.populate(&list)
    }

    Ok(tree)
}

// FIXME: As far as I can tell, this populates the tree after getting context when it should be
// doing it before.
pub fn view(connection: &Connection, id: i32, context: u32) -> Result<Option<Node>, DieselError> {
    use crate::schema::comments::dsl;

    // Get article id from comment with matching id
    let article_id = dsl::comments.find(id).first::<Comment>(connection)?.article;
    let list = list_flat(connection, article_id)?;

    let mut comment = list.iter().find(|comment| comment.id == id);

    // Replace comment with its parent context times (if the parent exists)
    for _ in 0..context {
        comment = list
            .iter()
            .find(|parent| comment.and_then(|comment| comment.parent) == Some(parent.id));
    }

    let mut node = comment.cloned().map(Node::new);
    if let Some(node) = node.as_mut() {
        node.populate(&list)
    };

    Ok(node)
}

pub fn view_single(connection: &Connection, id: i32) -> Result<Option<Comment>, DieselError> {
    use crate::schema::comments::dsl;

    dsl::comments.find(id).first(connection).optional()
}

pub fn submit(connection: &Connection, comment: NewComment) -> Result<Comment, DieselError> {
    let sumbitted = diesel::insert_into(comments::table)
        .values(&comment)
        .get_result(connection)?;
    Ok(sumbitted)
}

pub fn edit(
    connection: &Connection,
    id: i32,
    changes: CommentChanges,
) -> Result<usize, DieselError> {
    use crate::schema::comments::dsl;

    diesel::update(dsl::comments.find(id))
        .set(&changes)
        .execute(connection)
}

pub fn delete(connection: &Connection, id: i32) -> Result<usize, DieselError> {
    use crate::schema::comments::dsl;

    diesel::update(dsl::comments.find(id))
        .set(dsl::visible.eq(false))
        .execute(connection)
}

/// Delete a comment from the database. The target comment must have no direct children.
pub fn purge(connection: &Connection, id: i32) -> Result<usize, failure::Error> {
    use crate::schema::comments::dsl;

    // Check if the comment has any direct children, and refuse to purge it if so.
    // TODO: Explicit error
    let children_count: i64 = dsl::comments
        .filter(dsl::parent.eq(id))
        .count()
        .first(connection)?;
    if children_count == 0 {
        Ok(diesel::delete(dsl::comments.find(id)).execute(connection)?)
    } else {
        Err(failure::err_msg("Can't purge comment with direct children"))
    }
}

pub fn author(connection: &Connection, id: i32) -> Result<Option<String>, DieselError> {
    use crate::schema::comments::dsl;

    dsl::comments.select(dsl::author).find(id).first(connection)
}

pub fn by_user(connection: &Connection, user: &str) -> Result<Vec<Comment>, DieselError> {
    use crate::schema::comments::dsl;

    dsl::comments.filter(dsl::author.eq(user)).load(connection)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{Comment, Node};

    fn new(id: i32, parent: Option<i32>) -> Comment {
        Comment {
            id,
            parent,
            article: 1,
            author: Some(String::from("test_author")),
            name: None,
            content: String::from("Test article"),
            date: Utc::now().naive_utc(),
            visible: true,
        }
    }

    #[test]
    fn json_encode() {
        let comment = new(1, None);
        let tree = Node {
            comment: comment.clone(),
            children: vec![
                Node {
                    comment: comment.clone(),
                    children: Vec::new(),
                },
                Node {
                    comment: comment.clone(),
                    children: Vec::new(),
                },
            ],
        };
        let json = serde_json::to_string_pretty(&tree).unwrap();
        println!("{}", json);
    }
}
