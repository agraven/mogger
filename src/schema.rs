table! {
    articles (id) {
        id -> Int4,
        title -> Varchar,
        author -> Varchar,
        url -> Varchar,
        content -> Text,
        date -> Timestamp,
        visible -> Bool,
    }
}

table! {
    comments (id) {
        id -> Int4,
        parent -> Nullable<Int4>,
        article -> Int4,
        author -> Nullable<Varchar>,
        name -> Nullable<Varchar>,
        content -> Text,
        date -> Timestamp,
        visible -> Bool,
    }
}

table! {
    groups (id) {
        id -> Varchar,
        permissions -> Array<crate::user::PermissionMapping>,
    }
}

table! {
    sessions (id) {
        id -> Varchar,
        user -> Varchar,
        expires -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Varchar,
        hash -> Varchar,
        salt -> Bytea,
        name -> Varchar,
        email -> Varchar,
        group -> Varchar,
        rehash -> Bool,
    }
}

joinable!(articles -> users (author));
joinable!(comments -> articles (article));
joinable!(comments -> users (author));
joinable!(sessions -> users (user));
joinable!(users -> groups (group));

allow_tables_to_appear_in_same_query!(articles, comments, groups, sessions, users,);
