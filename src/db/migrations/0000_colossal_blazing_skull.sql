CREATE TABLE `audit_logs` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`action` text NOT NULL,
	`table_name` text NOT NULL,
	`record_id` text,
	`details` text,
	`created_at` text,
	`updated_at` text,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `audit_revisions` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text,
	`table_name` text NOT NULL,
	`record_id` text NOT NULL,
	`operation` text NOT NULL,
	`old_values` text,
	`new_values` text,
	`created_at` text,
	`updated_at` text,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `charities` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`name` text NOT NULL,
	`ein` text,
	`category` text,
	`status` text,
	`classification` text,
	`nonprofit_type` text,
	`deductibility` text,
	`street` text,
	`city` text,
	`state` text,
	`zip` text,
	`is_encrypted` integer DEFAULT false,
	`encrypted_payload` text,
	`created_at` text,
	`updated_at` text,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `donations` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`donation_year` integer,
	`donation_date` text,
	`donation_category` text,
	`donation_amount` real,
	`charity_id` text NOT NULL,
	`notes` text,
	`is_encrypted` integer DEFAULT false,
	`encrypted_payload` text,
	`created_at` text,
	`updated_at` text,
	`deleted` integer DEFAULT false,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`charity_id`) REFERENCES `charities`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
CREATE TABLE `receipts` (
	`id` text PRIMARY KEY NOT NULL,
	`donation_id` text NOT NULL,
	`receipt_key` text NOT NULL,
	`file_name` text,
	`content_type` text,
	`receipt_size` integer,
	`ocr_text` text,
	`ocr_date` text,
	`ocr_amount` real,
	`ocr_status` text,
	`is_encrypted` integer DEFAULT false,
	`encrypted_payload` text,
	`created_at` text,
	`updated_at` text,
	FOREIGN KEY (`donation_id`) REFERENCES `donations`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `users` (
	`id` text PRIMARY KEY NOT NULL,
	`email` text NOT NULL,
	`name` text,
	`filing_status` text,
	`agi` real,
	`marginal_tax_rate` real,
	`itemize_deductions` integer,
	`provider` text,
	`is_encrypted` integer DEFAULT false,
	`encrypted_payload` text,
	`vault_credential_id` text,
	`created_at` text,
	`updated_at` text
);
--> statement-breakpoint
CREATE UNIQUE INDEX `users_email_unique` ON `users` (`email`);--> statement-breakpoint
CREATE TABLE `val_categories` (
	`id` text PRIMARY KEY NOT NULL,
	`name` text NOT NULL,
	`description` text,
	`created_at` text,
	`updated_at` text
);
--> statement-breakpoint
CREATE TABLE `val_items` (
	`id` text PRIMARY KEY NOT NULL,
	`category_id` text,
	`name` text NOT NULL,
	`suggested_min` real,
	`suggested_max` real,
	`created_at` text,
	`updated_at` text,
	FOREIGN KEY (`category_id`) REFERENCES `val_categories`(`id`) ON UPDATE no action ON DELETE cascade
);
