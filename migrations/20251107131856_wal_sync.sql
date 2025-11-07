-- Add migration script here
CREATE TYPE wal_operation AS ENUM ('SET', 'DELETE');

CREATE TABLE IF NOT EXISTS wal_sync (
    key VARCHAR(100) PRIMARY KEY NOT NULL,
    time NUMERIC(39, 0) NOT NULL,
    operation wal_operation NOT NULL,
    value TEXT
);