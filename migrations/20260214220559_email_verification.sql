-- Add email verification fields to users table
ALTER TABLE users 
ADD COLUMN email_verified BOOLEAN DEFAULT false NOT NULL,
ADD COLUMN email_verification_token TEXT UNIQUE,
ADD COLUMN email_verification_token_expires_at TIMESTAMPTZ;

CREATE INDEX idx_users_verification_token ON users(email_verification_token) 
WHERE email_verification_token IS NOT NULL;