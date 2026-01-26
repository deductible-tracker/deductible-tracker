-- Users Table
CREATE TABLE users (
    id VARCHAR2(255) PRIMARY KEY,
    email VARCHAR2(255) NOT NULL UNIQUE,
    name VARCHAR2(255),
    provider VARCHAR2(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Donations Table
CREATE TABLE donations (
    id VARCHAR2(255) PRIMARY KEY,
    user_id VARCHAR2(255) NOT NULL,
    donation_year NUMBER(4),
    donation_date DATE,
    charity_id VARCHAR2(255),
    charity_name VARCHAR2(255),
    charity_ein VARCHAR2(50),
    notes VARCHAR2(4000),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    deleted NUMBER(1) DEFAULT 0,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Index for querying donations by user and year
CREATE INDEX idx_donations_user_year ON donations(user_id, donation_year);

-- Default Users for testing
INSERT INTO users (id, email, name, provider) VALUES ('dev-1', 'dev@local', 'Developer', 'local');
INSERT INTO users (id, email, name, provider) VALUES ('user-123', 'test@example.com', 'Test User', 'local');
