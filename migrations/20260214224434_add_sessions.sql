-- Sessions table for cookie-based authentication
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_sessions_expiry ON sessions(expiry_date);