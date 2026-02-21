-- Users Table
CREATE TABLE users (
    id VARCHAR2(255) PRIMARY KEY,
    email VARCHAR2(255) NOT NULL UNIQUE,
    name VARCHAR2(255),
    phone VARCHAR2(64),
    tax_id VARCHAR2(64),
    filing_status VARCHAR2(32),
    agi NUMBER(14,2),
    marginal_tax_rate NUMBER(6,4),
    itemize_deductions NUMBER(1),
    provider VARCHAR2(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

-- Donations Table
CREATE TABLE donations (
    id VARCHAR2(255) PRIMARY KEY,
    user_id VARCHAR2(255) NOT NULL,
    donation_year NUMBER(4),
    donation_date DATE,
    donation_category VARCHAR2(32),
    donation_amount NUMBER(12,2),
    charity_id VARCHAR2(255) NOT NULL,
    notes VARCHAR2(4000),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    deleted NUMBER(1) DEFAULT 0,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Index for querying donations by user and year
CREATE INDEX idx_donations_user_year ON donations(user_id, donation_year);
CREATE INDEX idx_donations_user_updated_created ON donations(user_id, updated_at, created_at);

-- Charities Table
CREATE TABLE charities (
    id VARCHAR2(255) PRIMARY KEY,
    user_id VARCHAR2(255) NOT NULL,
    name VARCHAR2(255) NOT NULL,
    ein VARCHAR2(50),
    category VARCHAR2(255),
    status VARCHAR2(255),
    classification VARCHAR2(255),
    nonprofit_type VARCHAR2(255),
    deductibility VARCHAR2(64),
    street VARCHAR2(255),
    city VARCHAR2(120),
    state VARCHAR2(16),
    zip VARCHAR2(20),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT fk_charities_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_charities_user ON charities(user_id);
CREATE UNIQUE INDEX idx_charities_user_name ON charities(user_id, LOWER(name));
ALTER TABLE donations ADD CONSTRAINT fk_donations_charity FOREIGN KEY (charity_id) REFERENCES charities(id);

-- Receipts Table
CREATE TABLE receipts (
    id VARCHAR2(255) PRIMARY KEY,
    donation_id VARCHAR2(255) NOT NULL,
    key VARCHAR2(1024) NOT NULL,
    file_name VARCHAR2(1024),
    content_type VARCHAR2(255),
    size NUMBER,
    ocr_text CLOB,
    ocr_date DATE,
    ocr_amount NUMBER,
    ocr_status VARCHAR2(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT fk_receipts_donation FOREIGN KEY (donation_id) REFERENCES donations(id)
);

CREATE INDEX idx_receipts_donation ON receipts(donation_id);
CREATE INDEX idx_receipts_donation_key ON receipts(donation_id, key);

-- OCR extraction/table for receipts (fields embedded in receipts above)

-- Valuation suggestion tables
CREATE TABLE val_categories (
    id VARCHAR2(255) PRIMARY KEY,
    name VARCHAR2(255) NOT NULL,
    description VARCHAR2(2000),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE TABLE val_items (
    id VARCHAR2(255) PRIMARY KEY,
    category_id VARCHAR2(255),
    name VARCHAR2(1024) NOT NULL,
    suggested_min NUMBER,
    suggested_max NUMBER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT fk_val_category FOREIGN KEY (category_id) REFERENCES val_categories(id)
);

-- Audit log for CPA/export
CREATE TABLE audit_logs (
    id VARCHAR2(255) PRIMARY KEY,
    user_id VARCHAR2(255) NOT NULL,
    action VARCHAR2(50) NOT NULL,
    table_name VARCHAR2(255) NOT NULL,
    record_id VARCHAR2(255),
    details CLOB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT fk_audit_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE audit_revisions (
    id VARCHAR2(255) PRIMARY KEY,
    user_id VARCHAR2(255),
    table_name VARCHAR2(255) NOT NULL,
    record_id VARCHAR2(255) NOT NULL,
    operation VARCHAR2(16) NOT NULL,
    old_values CLOB,
    new_values CLOB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT fk_audit_revisions_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at);

-- Default Users for testing
MERGE INTO users t
USING (SELECT 'dev-1' id, 'dev@local' email, 'Developer' name, 'local' provider FROM dual) s
ON (t.id = s.id)
WHEN NOT MATCHED THEN
    INSERT (id, email, name, provider) VALUES (s.id, s.email, s.name, s.provider);

MERGE INTO users t
USING (SELECT 'user-123' id, 'test@example.com' email, 'Test User' name, 'local' provider FROM dual) s
ON (t.id = s.id)
WHEN NOT MATCHED THEN
    INSERT (id, email, name, provider) VALUES (s.id, s.email, s.name, s.provider);
