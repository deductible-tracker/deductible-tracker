import { sqliteTable, text, integer, real } from "drizzle-orm/sqlite-core";

export const users = sqliteTable("users", {
  id: text("id").primaryKey(),
  email: text("email").notNull().unique(),
  name: text("name"),
  filingStatus: text("filing_status"),
  agi: real("agi"),
  marginalTaxRate: real("marginal_tax_rate"),
  itemizeDeductions: integer("itemize_deductions", { mode: "boolean" }),
  provider: text("provider"),
  isEncrypted: integer("is_encrypted", { mode: "boolean" }).default(false),
  encryptedPayload: text("encrypted_payload"),
  vaultCredentialId: text("vault_credential_id"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const charities = sqliteTable("charities", {
  id: text("id").primaryKey(),
  userId: text("user_id").notNull().references(() => users.id, { onDelete: "cascade" }),
  name: text("name").notNull(),
  ein: text("ein"),
  category: text("category"),
  status: text("status"),
  classification: text("classification"),
  nonprofitType: text("nonprofit_type"),
  deductibility: text("deductibility"),
  street: text("street"),
  city: text("city"),
  state: text("state"),
  zip: text("zip"),
  isEncrypted: integer("is_encrypted", { mode: "boolean" }).default(false),
  encryptedPayload: text("encrypted_payload"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const donations = sqliteTable("donations", {
  id: text("id").primaryKey(),
  userId: text("user_id").notNull().references(() => users.id, { onDelete: "cascade" }),
  donationYear: integer("donation_year"),
  donationDate: text("donation_date"), // YYYY-MM-DD
  donationCategory: text("donation_category"),
  donationAmount: real("donation_amount"),
  charityId: text("charity_id").notNull().references(() => charities.id),
  notes: text("notes"),
  isEncrypted: integer("is_encrypted", { mode: "boolean" }).default(false),
  encryptedPayload: text("encrypted_payload"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
  deleted: integer("deleted", { mode: "boolean" }).default(false),
});

export const receipts = sqliteTable("receipts", {
  id: text("id").primaryKey(),
  donationId: text("donation_id").notNull().references(() => donations.id, { onDelete: "cascade" }),
  receiptKey: text("receipt_key").notNull(),
  fileName: text("file_name"),
  contentType: text("content_type"),
  receiptSize: integer("receipt_size"),
  ocrText: text("ocr_text"),
  ocrDate: text("ocr_date"), // YYYY-MM-DD
  ocrAmount: real("ocr_amount"),
  ocrStatus: text("ocr_status"),
  isEncrypted: integer("is_encrypted", { mode: "boolean" }).default(false),
  encryptedPayload: text("encrypted_payload"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const valCategories = sqliteTable("val_categories", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  description: text("description"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const valItems = sqliteTable("val_items", {
  id: text("id").primaryKey(),
  categoryId: text("category_id").references(() => valCategories.id, { onDelete: "cascade" }),
  name: text("name").notNull(),
  suggestedMin: real("suggested_min"),
  suggestedMax: real("suggested_max"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const auditLogs = sqliteTable("audit_logs", {
  id: text("id").primaryKey(),
  userId: text("user_id").notNull().references(() => users.id, { onDelete: "cascade" }),
  action: text("action").notNull(),
  tableName: text("table_name").notNull(),
  recordId: text("record_id"),
  details: text("details"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});

export const auditRevisions = sqliteTable("audit_revisions", {
  id: text("id").primaryKey(),
  userId: text("user_id").references(() => users.id, { onDelete: "cascade" }),
  tableName: text("table_name").notNull(),
  recordId: text("record_id").notNull(),
  operation: text("operation").notNull(),
  oldValues: text("old_values"),
  newValues: text("new_values"),
  createdAt: text("created_at").$defaultFn(() => new Date().toISOString()),
  updatedAt: text("updated_at"),
});
