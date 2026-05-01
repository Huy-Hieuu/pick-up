-- Add password_hash to users for login authentication
ALTER TABLE users ADD COLUMN password_hash TEXT;