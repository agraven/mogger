ALTER TABLE users ADD COLUMN rehash BOOLEAN NOT NULL DEFAULT false;
UPDATE users SET rehash = true;
DELETE FROM sessions;
