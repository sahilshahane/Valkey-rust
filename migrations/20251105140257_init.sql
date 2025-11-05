-- Add migration script here
CREATE TABLE IF NOT EXISTS kv_store (
    key VARCHAR(100) PRIMARY KEY NOT NULL,
    value VARCHAR(100) NOT NULL
);