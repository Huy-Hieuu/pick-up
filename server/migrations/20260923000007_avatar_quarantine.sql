CREATE TABLE avatar_uploads (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    object_key TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL,
    content_length BIGINT NOT NULL CHECK (content_length BETWEEN 1 AND 8388608),
    expires_at TIMESTAMPTZ NOT NULL,
    processing_until TIMESTAMPTZ,
    public_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_avatar_uploads_owner ON avatar_uploads (user_id);
-- Retain old references for an operator-led revalidation, but stop serving
-- unvalidated images from profiles. No storage objects are deleted here.
CREATE TABLE legacy_avatar_references AS
    SELECT id AS user_id, avatar_url, now() AS archived_at FROM users WHERE avatar_url IS NOT NULL;
UPDATE users SET avatar_url = NULL WHERE avatar_url IS NOT NULL;
