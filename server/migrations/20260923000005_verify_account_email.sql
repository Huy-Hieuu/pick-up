-- A new account's email becomes a login/password-reset identity only after
-- OTP proof. Legacy emails are explicitly treated as unverified.
ALTER TABLE users
    ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;

-- Previously issued access tokens for email accounts did not prove email
-- ownership. Force re-authentication so the new verified-email rule takes effect.
DELETE FROM auth_sessions s
USING users u
WHERE s.user_id = u.id AND u.email IS NOT NULL;

CREATE INDEX idx_users_verified_email ON users (email) WHERE email_verified;
