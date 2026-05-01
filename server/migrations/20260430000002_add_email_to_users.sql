-- Add email field to users table for OTP delivery
ALTER TABLE users ADD COLUMN email TEXT UNIQUE;

-- Index for fast email lookups (used in OTP flow)
CREATE INDEX idx_users_email ON users (email) WHERE email IS NOT NULL;