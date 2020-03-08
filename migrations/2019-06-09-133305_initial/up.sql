CREATE TYPE permission AS ENUM (
	'all',
	'create_article',
	'edit_article',
	'delete_article',
	'edit_foreign_article',
	'delete_foreign_article',
	'create_comment',
	'edit_comment',
	'delete_comment',
	'edit_foreign_comment',
	'delete_foreign_comment',
	'create_user',
	'edit_foreign_user',
	'delete_foreign_user'
);

CREATE TABLE groups (
	id VARCHAR(255) PRIMARY KEY NOT NULL,
	permissions permission[] NOT NULL
);
CREATE TABLE users (
	id VARCHAR(255) PRIMARY KEY NOT NULL,
	hash VARCHAR(255) NOT NULL,
	salt bytea NOT NULL,
	name VARCHAR(255) NOT NULL,
	email VARCHAR(255) NOT NULL,
	"group" VARCHAR(255) REFERENCES groups(id) NOT NULL
);
CREATE TABLE articles (
	id SERIAL PRIMARY KEY NOT NULL,
	title VARCHAR(255) NOT NULL,
	author VARCHAR(255) REFERENCES users(id) NOT NULL,
	url VARCHAR(255) NOT NULL UNIQUE,
	content TEXT NOT NULL,
	date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	visible BOOLEAN NOT NULL
);
CREATE TABLE sessions (
	id VARCHAR(255) PRIMARY KEY NOT NULL,
	"user" VARCHAR(255) REFERENCES users(id) NOT NULL,
	expires TIMESTAMP NOT NULL
);
CREATE TABLE comments (
	id SERIAL PRIMARY KEY NOT NULL,
	parent INTEGER,
	article INTEGER REFERENCES articles(id) NOT NULL,
	author VARCHAR(255) REFERENCES users(id),
	name VARCHAR(255),
	content TEXT NOT NULL,
	date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	visible BOOLEAN NOT NULL,
	-- Must author xor name
	CONSTRAINT chk_author CHECK ((author IS NULL) <> (name IS NULL))
);

INSERT INTO groups (id, permissions) VALUES 
('admin', '{all}'),
('author', '{create_article, edit_article, delete_article, create_comment, edit_comment, edit_foreign_comment, delete_comment, delete_foreign_comment}'),
('default', '{create_comment, edit_comment, delete_comment}');

