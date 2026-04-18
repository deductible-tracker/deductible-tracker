use deadpool_oracle::Pool;

pub(crate) async fn run_bootstrap_ddl(pool: &Pool) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    for sql in [
        "ALTER TABLE users ADD (filing_status VARCHAR2(32))",
        "ALTER TABLE users ADD (agi NUMBER(14,2))",
        "ALTER TABLE users ADD (marginal_tax_rate NUMBER(6,4))",
        "ALTER TABLE users ADD (itemize_deductions NUMBER(1))",
        "ALTER TABLE users ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE users ADD (encrypted_payload CLOB)",
        "ALTER TABLE users ADD (vault_credential_id VARCHAR2(512))",
        "ALTER TABLE users ADD (updated_at TIMESTAMP)",
        "ALTER TABLE charities ADD (category VARCHAR2(255))",
        "ALTER TABLE charities ADD (status VARCHAR2(255))",
        "ALTER TABLE charities ADD (classification VARCHAR2(255))",
        "ALTER TABLE charities ADD (nonprofit_type VARCHAR2(255))",
        "ALTER TABLE charities ADD (deductibility VARCHAR2(64))",
        "ALTER TABLE charities ADD (street VARCHAR2(255))",
        "ALTER TABLE charities ADD (city VARCHAR2(120))",
        "ALTER TABLE charities ADD (state VARCHAR2(16))",
        "ALTER TABLE charities ADD (zip VARCHAR2(20))",
        "ALTER TABLE charities ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE charities ADD (encrypted_payload CLOB)",
        "ALTER TABLE donations ADD (donation_category VARCHAR2(32))",
        "ALTER TABLE donations ADD (donation_amount NUMBER(12,2))",
        "ALTER TABLE donations ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE donations ADD (encrypted_payload CLOB)",
        "ALTER TABLE receipts ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE receipts ADD (encrypted_payload CLOB)",
        "ALTER TABLE receipts ADD (updated_at TIMESTAMP)",
        "ALTER TABLE audit_logs ADD (updated_at TIMESTAMP)",
        "ALTER TABLE val_categories ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
        "ALTER TABLE val_categories ADD (updated_at TIMESTAMP)",
        "ALTER TABLE val_items ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
        "ALTER TABLE val_items ADD (updated_at TIMESTAMP)",
        "CREATE TABLE audit_revisions (id VARCHAR2(255) PRIMARY KEY, user_id VARCHAR2(255), table_name VARCHAR2(255) NOT NULL, record_id VARCHAR2(255) NOT NULL, operation VARCHAR2(16) NOT NULL, old_values VARCHAR2(4000), new_values VARCHAR2(4000), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP, CONSTRAINT fk_audit_revisions_user FOREIGN KEY (user_id) REFERENCES users(id))",
        "CREATE INDEX idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at)",
        "CREATE INDEX idx_charities_user_ein ON charities(user_id, ein)",
        "CREATE INDEX idx_val_items_category_name ON val_items(category_id, name)",
        "CREATE INDEX idx_val_items_lower_name ON val_items(LOWER(name))",
    ] {
        let _ = conn.execute(sql, &[]).await;
    }
    let _ = conn.commit().await;
    Ok(())
}
