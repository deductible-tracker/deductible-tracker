import { Hono } from "hono";
import { cors } from "hono/cors";
import { getCookie, setCookie, deleteCookie } from "hono/cookie";
import { getDb } from "./db";
import { users, donations, charities, receipts, valCategories, valItems, auditLogs, auditRevisions } from "./db/schema";
import { eq, and, desc, sql, inArray, lte } from "drizzle-orm";
import { sign, verify } from "hono/jwt";
import { presignUrl, normalizeObjectKey, userReceiptPrefix } from "./services/storage";
import JSZip from "jszip";

type Bindings = {
  DB: D1Database;
  JWT_SECRET: string;
  ALLOW_DEV_LOGIN?: string;
  DEV_USERNAME?: string;
  DEV_PASSWORD?: string;
  MISTRAL_API_KEY?: string;
  MISTRAL_API_ENDPOINT?: string;
  MISTRAL_MODEL?: string;
  OBJECT_STORAGE_ENDPOINT?: string;
  OBJECT_STORAGE_BUCKET?: string;
  OCI_REGION?: string;
  OCI_ACCESS_KEY_ID?: string;
  OCI_SECRET_ACCESS_KEY?: string;
  PROPUBLICA_API_BASE_URL?: string;
  GOOGLE_CLIENT_ID?: string;
  GOOGLE_CLIENT_SECRET?: string;
};

type Variables = {
  user?: {
    id: string;
    email: string;
    name: string;
    provider: string;
  };
};

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

// ==========================================
// DB Bootstrap Helper (SQLite DDL auto-apply)
// ==========================================
async function bootstrapDb(d1: D1Database) {
  // Check if users table exists
  try {
    await d1.prepare("SELECT 1 FROM users LIMIT 1").first();
  } catch (e) {
    console.log("Database tables missing. Bootstrapping schema...");
    
    // Create tables in order
    const queries = [
      `CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        email TEXT NOT NULL UNIQUE,
        name TEXT,
        filing_status TEXT,
        agi REAL,
        marginal_tax_rate REAL,
        itemize_deductions INTEGER,
        provider TEXT,
        is_encrypted INTEGER DEFAULT 0,
        encrypted_payload TEXT,
        vault_credential_id TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT
      )`,
      `CREATE TABLE IF NOT EXISTS charities (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        name TEXT NOT NULL,
        ein TEXT,
        category TEXT,
        status TEXT,
        classification TEXT,
        nonprofit_type TEXT,
        deductibility TEXT,
        street TEXT,
        city TEXT,
        state TEXT,
        zip TEXT,
        is_encrypted INTEGER DEFAULT 0,
        encrypted_payload TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
      )`,
      `CREATE TABLE IF NOT EXISTS donations (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        donation_year INTEGER,
        donation_date TEXT,
        donation_category TEXT,
        donation_amount REAL,
        charity_id TEXT NOT NULL,
        notes TEXT,
        is_encrypted INTEGER DEFAULT 0,
        encrypted_payload TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        deleted INTEGER DEFAULT 0,
        FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
        FOREIGN KEY(charity_id) REFERENCES charities(id)
      )`,
      `CREATE TABLE IF NOT EXISTS receipts (
        id TEXT PRIMARY KEY,
        donation_id TEXT NOT NULL,
        receipt_key TEXT NOT NULL,
        file_name TEXT,
        content_type TEXT,
        receipt_size INTEGER,
        ocr_text TEXT,
        ocr_date TEXT,
        ocr_amount REAL,
        ocr_status TEXT,
        is_encrypted INTEGER DEFAULT 0,
        encrypted_payload TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        FOREIGN KEY(donation_id) REFERENCES donations(id) ON DELETE CASCADE
      )`,
      `CREATE TABLE IF NOT EXISTS val_categories (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT
      )`,
      `CREATE TABLE IF NOT EXISTS val_items (
        id TEXT PRIMARY KEY,
        category_id TEXT,
        name TEXT NOT NULL,
        suggested_min REAL,
        suggested_max REAL,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        FOREIGN KEY(category_id) REFERENCES val_categories(id) ON DELETE CASCADE
      )`,
      `CREATE TABLE IF NOT EXISTS audit_logs (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        action TEXT NOT NULL,
        table_name TEXT NOT NULL,
        record_id TEXT,
        details TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
      )`,
      `CREATE TABLE IF NOT EXISTS audit_revisions (
        id TEXT PRIMARY KEY,
        user_id TEXT,
        table_name TEXT NOT NULL,
        record_id TEXT NOT NULL,
        operation TEXT NOT NULL,
        old_values TEXT,
        new_values TEXT,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT,
        FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
      )`,
      // Seed default testing accounts
      `INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('dev-1', 'dev@local', 'Developer', 'local')`,
      `INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('user-123', 'test@example.com', 'Test User', 'local')`
    ];

    for (const q of queries) {
      await d1.prepare(q).run();
    }
    console.log("Database bootstrap complete.");
  }
}

// ==========================================
// Middlewares
// ==========================================

// Global CORS configuration matching the original setup
app.use("*", async (c, next) => {
  const origin = c.req.header("Origin");
  let allowedOrigin = "";
  if (origin) {
    try {
      const url = new URL(origin);
      if (
        url.hostname === "localhost" ||
        url.hostname === "127.0.0.1" ||
        url.hostname.includes("deductible-tracker")
      ) {
        allowedOrigin = origin;
      }
    } catch {
      // ignore
    }
  }
  if (allowedOrigin) {
    c.res.headers.set("Access-Control-Allow-Origin", allowedOrigin);
    c.res.headers.set("Access-Control-Allow-Credentials", "true");
  }
  c.res.headers.set("Vary", "Origin");

  if (c.req.method === "OPTIONS") {
    c.res.headers.set("Access-Control-Allow-Methods", "GET, HEAD, PUT, POST, DELETE, PATCH");
    const reqHeaders = c.req.header("Access-Control-Request-Headers");
    if (reqHeaders) {
      c.res.headers.set("Access-Control-Allow-Headers", reqHeaders);
    }
    return c.text("", 204);
  }
  await next();
});

// Database boostrapper middleware
app.use("*", async (c, next) => {
  await bootstrapDb(c.env.DB);
  await next();
});

// CSRF check middleware (state-changing API routes require csrf_token verification)
app.use("*", async (c, next) => {
  const path = c.req.path;
  const method = c.req.method;

  if (
    path.startsWith("/api/") &&
    ["POST", "PUT", "DELETE", "PATCH"].includes(method) &&
    path !== "/api/config"
  ) {
    const csrfHeader = c.req.header("X-CSRF-Token");
    const csrfCookie = getCookie(c, "csrf_token");

    if (!csrfHeader || !csrfCookie || csrfHeader !== csrfCookie) {
      console.warn("CSRF token validation failed");
      return c.text("CSRF token mismatch or missing", 403);
    }
  }
  await next();
});

// Authentication middleware
app.use("*", async (c, next) => {
  const path = c.req.path;

  // Let public auth routes & config pass through
  if (
    path.startsWith("/auth/") ||
    path === "/api/config" ||
    !path.startsWith("/api/")
  ) {
    return await next();
  }

  // Extract auth token from Header or Cookie
  let token = "";
  const authHeader = c.req.header("Authorization");
  if (authHeader && authHeader.startsWith("Bearer ")) {
    token = authHeader.substring(7);
  } else {
    token = getCookie(c, "auth_token") || "";
  }

  if (!token) {
    return c.text("Unauthorized", 401);
  }

  try {
    const secret = c.env.JWT_SECRET;
    const payload = await verify(token, secret);
    c.set("user", {
      id: payload.sub as string,
      email: payload.email as string,
      name: payload.name as string,
      provider: payload.provider as string,
    });
  } catch (e) {
    console.error("JWT verification failed:", e);
    return c.text("Unauthorized", 401);
  }

  await next();
});

// Audit logging helper
async function logAudit(
  db: any,
  userId: string,
  action: string,
  tableName: string,
  recordId: string | null,
  details: string | null
) {
  try {
    await db.insert(auditLogs).values({
      id: crypto.randomUUID(),
      userId,
      action,
      tableName,
      recordId,
      details,
    });
  } catch (e) {
    console.error("Failed to write audit log:", e);
  }
}

// Audit revision helper
async function logRevision(
  db: any,
  userId: string | null,
  tableName: string,
  recordId: string,
  operation: string,
  oldValues: string | null,
  newValues: string | null
) {
  try {
    await db.insert(auditRevisions).values({
      id: crypto.randomUUID(),
      userId,
      tableName,
      recordId,
      operation,
      oldValues,
      newValues,
    });
  } catch (e) {
    console.error("Failed to write revision log:", e);
  }
}

// Helper to sign JWTs
async function createJwt(user: { id: string; email: string; name: string; provider: string }, secret: string) {
  const payload = {
    sub: user.id,
    email: user.email,
    name: user.name,
    provider: user.provider,
    exp: Math.floor(Date.now() / 1000) + 24 * 60 * 60, // 24 hours
    jti: crypto.randomUUID(),
  };
  return await sign(payload, secret);
}

// ==========================================
// Authentication Routes
// ==========================================

// Config route
app.get("/api/config", (c) => {
  const allowDevLogin = c.env.ALLOW_DEV_LOGIN === "true";
  const googleEnabled = !!(c.env.GOOGLE_CLIENT_ID && c.env.GOOGLE_CLIENT_SECRET);
  const googleClientId = c.env.GOOGLE_CLIENT_ID || null;

  return c.json({
    allow_dev_login: allowDevLogin,
    google_enabled: googleEnabled,
    google_client_id: googleClientId,
    oauth_state: crypto.randomUUID(),
  });
});

// Dev Login
app.post("/auth/dev-login", async (c) => {
  const envMode = c.env.ALLOW_DEV_LOGIN === "true";
  if (!envMode) {
    return c.text("Dev login disabled", 403);
  }

  const { username, password } = await c.req.json();
  const devUser = c.env.DEV_USERNAME || "admin";
  const devPass = c.env.DEV_PASSWORD;

  if (!devPass || devPass === "password") {
    return c.text("Dev login misconfigured", 403);
  }

  if (username === devUser && password === devPass) {
    const db = getDb(c.env.DB);
    let userRecord = await db.select().from(users).where(eq(users.email, "dev@local")).get();

    if (!userRecord) {
      userRecord = {
        id: "dev-1",
        email: "dev@local",
        name: "Developer",
        provider: "local",
        filingStatus: null,
        agi: null,
        marginalTaxRate: null,
        itemizeDeductions: null,
        isEncrypted: false,
        encryptedPayload: null,
        vaultCredentialId: null,
        createdAt: new Date().toISOString(),
        updatedAt: null,
      };
      await db.insert(users).values(userRecord);
    }

    const token = await createJwt(
      {
        id: userRecord.id,
        email: userRecord.email,
        name: userRecord.name || "Developer",
        provider: userRecord.provider || "local",
      },
      c.env.JWT_SECRET
    );

    const secure = c.req.url.startsWith("https");
    setCookie(c, "auth_token", token, {
      httpOnly: true,
      path: "/",
      maxAge: 86400,
      secure,
      sameSite: "Strict",
    });
    setCookie(c, "csrf_token", token, {
      path: "/",
      maxAge: 86400,
      secure,
      sameSite: "Strict",
    });

    return c.json({
      user: {
        id: userRecord.id,
        email: userRecord.email,
        name: userRecord.name,
        provider: userRecord.provider,
        filing_status: userRecord.filingStatus,
        agi: userRecord.agi,
        marginal_tax_rate: userRecord.marginalTaxRate,
        itemize_deductions: userRecord.itemizeDeductions,
        is_encrypted: userRecord.isEncrypted,
        encrypted_payload: userRecord.encryptedPayload,
        vault_credential_id: userRecord.vaultCredentialId,
      },
    });
  }

  return c.text("Invalid credentials", 401);
});

// Google OAuth Authorization Redirect
app.get("/auth/login/google", (c) => {
  const clientId = c.env.GOOGLE_CLIENT_ID;
  const redirectUri = `${new URL(c.req.url).origin}/auth/callback/google`;
  if (!clientId) {
    return c.text("Google OAuth not configured", 400);
  }

  const oauthUrl = `https://accounts.google.com/o/oauth2/v2/auth?client_id=${clientId}&redirect_uri=${encodeURIComponent(
    redirectUri
  )}&response_type=code&scope=openid%20profile%20email&state=${crypto.randomUUID()}`;

  return c.redirect(oauthUrl);
});

// Google OAuth Callback Handler
app.get("/auth/callback/google", async (c) => {
  const code = c.req.query("code");
  if (!code) {
    return c.redirect("/login?error=missing_code");
  }

  const clientId = c.env.GOOGLE_CLIENT_ID;
  const clientSecret = c.env.GOOGLE_CLIENT_SECRET;
  const redirectUri = `${new URL(c.req.url).origin}/auth/callback/google`;

  try {
    // 1. Exchange code for access token
    const tokenResp = await fetch("https://oauth2.googleapis.com/token", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: `code=${code}&client_id=${clientId}&client_secret=${clientSecret}&redirect_uri=${encodeURIComponent(
        redirectUri
      )}&grant_type=authorization_code`,
    });

    const tokenData = (await tokenResp.json()) as any;
    if (!tokenData.access_token) {
      return c.redirect("/login?error=token_exchange_failed");
    }

    // 2. Fetch user profile from google userinfo API
    const userinfoResp = await fetch("https://www.googleapis.com/oauth2/v3/userinfo", {
      headers: { Authorization: `Bearer ${tokenData.access_token}` },
    });

    const profile = (await userinfoResp.json()) as any;
    const email = profile.email;
    const name = profile.name || profile.given_name || "Google User";
    const sub = profile.sub;

    if (!email) {
      return c.redirect("/login?error=email_not_provided");
    }

    const db = getDb(c.env.DB);
    let userRecord = await db.select().from(users).where(eq(users.email, email)).get();

    if (!userRecord) {
      userRecord = {
        id: sub,
        email,
        name,
        provider: "google",
        filingStatus: null,
        agi: null,
        marginalTaxRate: null,
        itemizeDeductions: null,
        isEncrypted: false,
        encryptedPayload: null,
        vaultCredentialId: null,
        createdAt: new Date().toISOString(),
        updatedAt: null,
      };
      await db.insert(users).values(userRecord);
    }

    const token = await createJwt(
      {
        id: userRecord.id,
        email: userRecord.email,
        name: userRecord.name || name,
        provider: userRecord.provider || "google",
      },
      c.env.JWT_SECRET
    );

    const secure = c.req.url.startsWith("https");
    setCookie(c, "auth_token", token, {
      httpOnly: true,
      path: "/",
      maxAge: 86400,
      secure,
      sameSite: "Strict",
    });
    setCookie(c, "csrf_token", token, {
      path: "/",
      maxAge: 86400,
      secure,
      sameSite: "Strict",
    });

    return c.redirect("/");
  } catch (err) {
    console.error("Google login failed:", err);
    return c.redirect("/login?error=internal_auth_error");
  }
});

// Logout Route
app.post("/auth/logout", (c) => {
  const secure = c.req.url.startsWith("https");
  deleteCookie(c, "auth_token", { path: "/", secure });
  deleteCookie(c, "csrf_token", { path: "/", secure });
  return c.text("OK");
});

// ==========================================
// User Profile API (/api/me)
// ==========================================

app.get("/api/me", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);
  const userRecord = await db.select().from(users).where(eq(users.id, user.id)).get();

  if (!userRecord) {
    return c.json({ authenticated: false }, 401);
  }

  return c.json({
    id: userRecord.id,
    email: userRecord.email,
    name: userRecord.name,
    provider: userRecord.provider,
    filing_status: userRecord.filingStatus,
    agi: userRecord.agi,
    marginal_tax_rate: userRecord.marginalTaxRate,
    itemize_deductions: userRecord.itemizeDeductions,
    is_encrypted: userRecord.isEncrypted,
    encrypted_payload: userRecord.encryptedPayload,
    vault_credential_id: userRecord.vaultCredentialId,
  });
});

app.put("/api/me", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const updates: any = {
    updatedAt: new Date().toISOString(),
  };
  if (body.name !== undefined) updates.name = body.name;
  if (body.filing_status !== undefined) updates.filingStatus = body.filing_status;
  if (body.agi !== undefined) updates.agi = body.agi;
  if (body.marginal_tax_rate !== undefined) updates.marginalTaxRate = body.marginal_tax_rate;
  if (body.itemize_deductions !== undefined) updates.itemizeDeductions = body.itemize_deductions;
  if (body.is_encrypted !== undefined) updates.isEncrypted = body.is_encrypted;
  if (body.encrypted_payload !== undefined) updates.encryptedPayload = body.encrypted_payload;
  if (body.vault_credential_id !== undefined) updates.vaultCredentialId = body.vault_credential_id;

  await db.update(users).set(updates).where(eq(users.id, user.id));

  const refreshed = await db.select().from(users).where(eq(users.id, user.id)).get();

  return c.json({
    id: refreshed.id,
    email: refreshed.email,
    name: refreshed.name,
    provider: refreshed.provider,
    filing_status: refreshed.filingStatus,
    agi: refreshed.agi,
    marginal_tax_rate: refreshed.marginalTaxRate,
    itemize_deductions: refreshed.itemizeDeductions,
    is_encrypted: refreshed.isEncrypted,
    encrypted_payload: refreshed.encryptedPayload,
    vault_credential_id: refreshed.vaultCredentialId,
  });
});

app.delete("/api/me", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);

  // Get receipt keys to clean storage
  const userPrefix = userReceiptPrefix(user.id);
  const receiptsList = await db
    .select({ key: receipts.receiptKey })
    .from(receipts)
    .innerJoin(donations, eq(donations.id, receipts.donationId))
    .where(eq(donations.userId, user.id))
    .all();

  // Try to delete receipts from storage (best effort)
  if (
    c.env.OBJECT_STORAGE_ENDPOINT &&
    c.env.OBJECT_STORAGE_BUCKET &&
    c.env.OCI_ACCESS_KEY_ID &&
    c.env.OCI_SECRET_ACCESS_KEY
  ) {
    for (const r of receiptsList) {
      try {
        const deleteUrl = await presignUrl(
          "DELETE",
          r.key,
          300,
          c.env as any
        );
        await fetch(deleteUrl, { method: "DELETE" });
      } catch (err) {
        console.error(`Failed to delete receipt ${r.key} from storage:`, err);
      }
    }
  }

  // Delete all database records (cascaded from user deletion)
  await db.delete(users).where(eq(users.id, user.id));

  // Logout
  const secure = c.req.url.startsWith("https");
  deleteCookie(c, "auth_token", { path: "/", secure });
  deleteCookie(c, "csrf_token", { path: "/", secure });

  return c.text("OK");
});

// ==========================================
// Backup & Export/Import Zip API
// ==========================================

app.get("/api/me/export", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);

  const profile = await db.select().from(users).where(eq(users.id, user.id)).get();
  const charitiesList = await db.select().from(charities).where(eq(charities.userId, user.id)).all();
  
  const donationsList = await db
    .select({
      id: donations.id,
      userId: donations.userId,
      year: donations.donationYear,
      date: donations.donationDate,
      category: donations.donationCategory,
      amount: donations.donationAmount,
      charityId: donations.charityId,
      notes: donations.notes,
      isEncrypted: donations.isEncrypted,
      encryptedPayload: donations.encryptedPayload,
      createdAt: donations.createdAt,
      updatedAt: donations.updatedAt,
      deleted: donations.deleted,
      charityName: charities.name,
      charityEin: charities.ein,
    })
    .from(donations)
    .innerJoin(charities, eq(charities.id, donations.charityId))
    .where(and(eq(donations.userId, user.id), eq(donations.deleted, false)))
    .all();

  const receiptsList = await db
    .select({
      id: receipts.id,
      donationId: receipts.donationId,
      receiptKey: receipts.receiptKey,
      fileName: receipts.fileName,
      contentType: receipts.contentType,
      receiptSize: receipts.receiptSize,
      ocrText: receipts.ocrText,
      ocrDate: receipts.ocrDate,
      ocrAmount: receipts.ocrAmount,
      ocrStatus: receipts.ocrStatus,
      isEncrypted: receipts.isEncrypted,
      encryptedPayload: receipts.encryptedPayload,
      createdAt: receipts.createdAt,
    })
    .from(receipts)
    .innerJoin(donations, eq(donations.id, receipts.donationId))
    .where(eq(donations.userId, user.id))
    .all();

  const dataJson = JSON.stringify({
    profile: {
      id: profile.id,
      email: profile.email,
      name: profile.name,
      provider: profile.provider,
      filing_status: profile.filingStatus,
      agi: profile.agi,
      marginal_tax_rate: profile.marginalTaxRate,
      itemize_deductions: profile.itemizeDeductions,
      is_encrypted: profile.isEncrypted,
      encrypted_payload: profile.encryptedPayload,
      vault_credential_id: profile.vaultCredentialId,
    },
    charities: charitiesList.map(c => ({
      id: c.id,
      user_id: c.userId,
      name: c.name,
      ein: c.ein,
      category: c.category,
      status: c.status,
      classification: c.classification,
      nonprofit_type: c.nonprofitType,
      deductibility: c.deductibility,
      street: c.street,
      city: c.city,
      state: c.state,
      zip: c.zip,
      is_encrypted: c.isEncrypted,
      encrypted_payload: c.encryptedPayload,
      created_at: c.createdAt,
      updated_at: c.updatedAt,
    })),
    donations: donationsList.map(d => ({
      id: d.id,
      user_id: d.userId,
      year: d.year,
      date: d.date,
      category: d.category,
      amount: d.amount,
      charity_id: d.charityId,
      notes: d.notes,
      is_encrypted: d.isEncrypted,
      encrypted_payload: d.encryptedPayload,
      created_at: d.createdAt,
      updated_at: d.updatedAt,
      deleted: d.deleted,
      charity_name: d.charityName,
      charity_ein: d.charityEin,
    })),
    receipts: receiptsList.map(r => ({
      id: r.id,
      donation_id: r.donationId,
      key: r.receiptKey,
      file_name: r.fileName,
      content_type: r.contentType,
      size: r.receiptSize,
      ocr_text: r.ocrText,
      ocr_date: r.ocrDate,
      ocr_amount: r.ocrAmount,
      ocr_status: r.ocrStatus,
      is_encrypted: r.isEncrypted,
      encrypted_payload: r.encryptedPayload,
      created_at: r.createdAt,
    })),
  });

  const zip = new JSZip();
  zip.file("data.json", dataJson);

  // Download all files from storage and bundle them into zip (best effort)
  if (
    c.env.OBJECT_STORAGE_ENDPOINT &&
    c.env.OBJECT_STORAGE_BUCKET &&
    c.env.OCI_ACCESS_KEY_ID &&
    c.env.OCI_SECRET_ACCESS_KEY
  ) {
    const receiptsFolder = zip.folder("receipts");
    for (const r of receiptsList) {
      try {
        const downloadUrl = await presignUrl(
          "GET",
          r.receiptKey,
          300,
          c.env as any
        );
        const resp = await fetch(downloadUrl);
        if (resp.ok) {
          const fileData = await resp.arrayBuffer();
          receiptsFolder?.file(r.fileName || r.id, fileData);
        }
      } catch (err) {
        console.error(`Failed to export receipt ${r.receiptKey}:`, err);
      }
    }
  }

  const zipData = await zip.generateAsync({ type: "uint8array" });

  c.res.headers.set("Content-Type", "application/zip");
  c.res.headers.set("Content-Disposition", `attachment; filename="backup-${user.id}-${new Date().toISOString().substring(0,10)}.zip"`);
  return c.body(zipData);
});

app.post("/api/me/import", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);
  const formData = await c.req.formData();
  const file = formData.get("file") as File;

  if (!file) {
    return c.text("Missing backup file", 400);
  }

  const zipBuffer = await file.arrayBuffer();
  const zip = await JSZip.loadAsync(zipBuffer);
  const dataJsonFile = zip.file("data.json");

  if (!dataJsonFile) {
    return c.text("Missing data.json in backup", 400);
  }

  const dataJsonRaw = await dataJsonFile.async("string");
  const backup = JSON.parse(dataJsonRaw);

  // Restore profile metadata
  await db
    .update(users)
    .set({
      filingStatus: backup.profile.filing_status,
      agi: backup.profile.agi,
      marginalTaxRate: backup.profile.marginal_tax_rate,
      itemizeDeductions: backup.profile.itemize_deductions,
      isEncrypted: backup.profile.is_encrypted,
      encryptedPayload: backup.profile.encrypted_payload,
      vaultCredentialId: backup.profile.vault_credential_id,
      updatedAt: new Date().toISOString(),
    })
    .where(eq(users.id, user.id));

  // Import charities
  for (const char of backup.charities) {
    const existing = await db.select().from(charities).where(eq(charities.id, char.id)).get();
    if (!existing) {
      await db.insert(charities).values({
        id: char.id,
        userId: user.id,
        name: char.name,
        ein: char.ein,
        category: char.category,
        status: char.status,
        classification: char.classification,
        nonprofitType: char.nonprofit_type,
        deductibility: char.deductibility,
        street: char.street,
        city: char.city,
        state: char.state,
        zip: char.zip,
        isEncrypted: char.is_encrypted,
        encryptedPayload: char.encrypted_payload,
        createdAt: char.created_at,
        updatedAt: char.updated_at,
      });
    }
  }

  // Import donations
  for (const don of backup.donations) {
    const existing = await db.select().from(donations).where(eq(donations.id, don.id)).get();
    if (!existing) {
      await db.insert(donations).values({
        id: don.id,
        userId: user.id,
        donationYear: don.year,
        donationDate: don.date,
        donationCategory: don.category,
        donationAmount: don.amount,
        charityId: don.charity_id,
        notes: don.notes,
        isEncrypted: don.is_encrypted,
        encryptedPayload: don.encrypted_payload,
        createdAt: don.created_at,
        updatedAt: don.updated_at,
        deleted: don.deleted,
      });
    }
  }

  // Import receipts
  for (const rec of backup.receipts) {
    const existing = await db.select().from(receipts).where(eq(receipts.id, rec.id)).get();
    if (!existing) {
      await db.insert(receipts).values({
        id: rec.id,
        donationId: rec.donation_id,
        receiptKey: rec.key,
        fileName: rec.file_name,
        contentType: rec.content_type,
        receiptSize: rec.size,
        ocrText: rec.ocr_text,
        ocrDate: rec.ocr_date,
        ocrAmount: rec.ocr_amount,
        ocrStatus: rec.ocr_status,
        isEncrypted: rec.is_encrypted,
        encryptedPayload: rec.encrypted_payload,
        createdAt: rec.created_at,
      });

      // Upload file back to storage
      if (
        c.env.OBJECT_STORAGE_ENDPOINT &&
        c.env.OBJECT_STORAGE_BUCKET &&
        c.env.OCI_ACCESS_KEY_ID &&
        c.env.OCI_SECRET_ACCESS_KEY
      ) {
        const fileInZip = zip.file(`receipts/${rec.file_name || rec.id}`);
        if (fileInZip) {
          try {
            const fileData = await fileInZip.async("uint8array");
            const uploadUrl = await presignUrl(
              "PUT",
              rec.key,
              300,
              c.env as any
            );
            await fetch(uploadUrl, {
              method: "PUT",
              headers: { "Content-Type": rec.content_type || "application/octet-stream" },
              body: fileData,
            });
          } catch (err) {
            console.error(`Failed to restore file ${rec.key} during import:`, err);
          }
        }
      }
    }
  }

  return c.text("Restore completed");
});

// ==========================================
// Donations API (/api/donations)
// ==========================================

app.get("/api/donations", async (c) => {
  const user = c.get("user")!;
  const since = c.req.query("since");
  const yearQuery = c.req.query("year");
  const db = getDb(c.env.DB);

  let query = db
    .select({
      id: donations.id,
      user_id: donations.userId,
      year: donations.donationYear,
      date: donations.donationDate,
      category: donations.donationCategory,
      amount: donations.donationAmount,
      charity_id: donations.charityId,
      notes: donations.notes,
      is_encrypted: donations.isEncrypted,
      encrypted_payload: donations.encryptedPayload,
      created_at: donations.createdAt,
      updated_at: donations.updatedAt,
      deleted: donations.deleted,
      charity_name: charities.name,
      charity_ein: charities.ein,
    })
    .from(donations)
    .innerJoin(charities, eq(charities.id, donations.charityId))
    .where(eq(donations.userId, user.id));

  if (since) {
    // Return all (including soft deleted) updated since pulling timestamp
    const list = await query.where(and(eq(donations.userId, user.id), sql`${donations.updatedAt} > ${since}`)).all();
    return c.json({ donations: list });
  } else {
    // Query normal donation list
    let condition = and(eq(donations.userId, user.id), eq(donations.deleted, false));
    if (yearQuery) {
      condition = and(condition, eq(donations.donationYear, parseInt(yearQuery, 10)));
    }
    const list = await query.where(condition).orderBy(desc(donations.donationDate)).all();
    return c.json({ donations: list });
  }
});

app.post("/api/donations", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const newDonation = {
    id: body.id || crypto.randomUUID(),
    userId: user.id,
    donationYear: body.year || new Date(body.date).getFullYear(),
    donationDate: body.date,
    donationCategory: body.category || "money",
    donationAmount: body.amount || null,
    charityId: body.charity_id,
    notes: body.notes || null,
    isEncrypted: !!body.is_encrypted,
    encryptedPayload: body.encrypted_payload || null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    deleted: false,
  };

  await db.insert(donations).values(newDonation);
  await logAudit(db, user.id, "create", "donations", newDonation.id, `Created donation to ${body.charity_id}`);

  return c.json({ id: newDonation.id }, 201);
});

app.put("/api/donations/:id", async (c) => {
  const user = c.get("user")!;
  const donationId = c.req.param("id");
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const existing = await db.select().from(donations).where(and(eq(donations.id, donationId), eq(donations.userId, user.id))).get();
  if (!existing) {
    return c.text("Not Found", 404);
  }

  const oldValues = JSON.stringify(existing);

  const updates: any = {
    updatedAt: new Date().toISOString(),
  };
  if (body.date !== undefined) {
    updates.donationDate = body.date;
    updates.donationYear = body.year || new Date(body.date).getFullYear();
  }
  if (body.category !== undefined) updates.donationCategory = body.category;
  if (body.amount !== undefined) updates.donationAmount = body.amount;
  if (body.charity_id !== undefined) updates.charityId = body.charity_id;
  if (body.notes !== undefined) updates.notes = body.notes;
  if (body.is_encrypted !== undefined) updates.isEncrypted = body.is_encrypted;
  if (body.encrypted_payload !== undefined) updates.encryptedPayload = body.encrypted_payload;

  await db.update(donations).set(updates).where(eq(donations.id, donationId));

  const updated = await db.select().from(donations).where(eq(donations.id, donationId)).get();
  await logRevision(db, user.id, "donations", donationId, "update", oldValues, JSON.stringify(updated));
  await logAudit(db, user.id, "update", "donations", donationId, "Updated donation fields");

  return c.json({ success: true });
});

app.delete("/api/donations/:id", async (c) => {
  const user = c.get("user")!;
  const donationId = c.req.param("id");
  const db = getDb(c.env.DB);

  const existing = await db.select().from(donations).where(and(eq(donations.id, donationId), eq(donations.userId, user.id))).get();
  if (!existing) {
    return c.text("Not Found", 404);
  }

  const oldValues = JSON.stringify(existing);

  // Soft delete donation
  await db
    .update(donations)
    .set({ deleted: true, updatedAt: new Date().toISOString() })
    .where(eq(donations.id, donationId));

  // Hard delete receipts associated with the donation
  await db.delete(receipts).where(eq(receipts.donationId, donationId));

  const updated = await db.select().from(donations).where(eq(donations.id, donationId)).get();
  await logRevision(db, user.id, "donations", donationId, "delete", oldValues, JSON.stringify(updated));
  await logAudit(db, user.id, "delete", "donations", donationId, "Soft deleted donation and deleted receipts");

  return c.json({ success: true });
});

// Offline synchronization batch endpoint (/api/sync/batch)
app.post("/api/sync/batch", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  // Process batch donations
  for (const d of body.donations || []) {
    if (d.action === "delete") {
      await db
        .update(donations)
        .set({ deleted: true, updatedAt: new Date().toISOString() })
        .where(and(eq(donations.id, d.id), eq(donations.userId, user.id)));
      await db.delete(receipts).where(eq(receipts.donationId, d.id));
      continue;
    }

    const donationDate = d.date;
    const donationYear = d.year || (donationDate ? new Date(donationDate).getFullYear() : new Date().getFullYear());

    const donationRecord = {
      id: d.id,
      userId: user.id,
      donationYear,
      donationDate,
      donationCategory: d.category || "money",
      donationAmount: d.amount || null,
      charityId: d.charity_id,
      notes: d.notes || null,
      isEncrypted: !!d.is_encrypted,
      encryptedPayload: d.encrypted_payload || null,
      updatedAt: d.updated_at || new Date().toISOString(),
    };

    // Merge/upsert using sqlite replace or custom check
    const existing = await db.select().from(donations).where(eq(donations.id, d.id)).get();
    if (!existing) {
      await db.insert(donations).values({ ...donationRecord, createdAt: new Date().toISOString() });
    } else {
      // Sync conflicts resolved by choosing the newest update
      const existingTime = existing.updatedAt ? new Date(existing.updatedAt).getTime() : 0;
      const incomingTime = donationRecord.updatedAt ? new Date(donationRecord.updatedAt).getTime() : Date.now();
      if (incomingTime >= existingTime) {
        await db.update(donations).set(donationRecord).where(eq(donations.id, d.id));
      }
    }
  }

  // Process batch receipts
  for (const r of body.receipts || []) {
    if (r.action !== "create") continue;
    const existing = await db.select().from(receipts).where(eq(receipts.id, r.id)).get();
    if (!existing) {
      await db.insert(receipts).values({
        id: r.id,
        donationId: r.donation_id,
        receiptKey: r.key,
        fileName: r.file_name || null,
        contentType: r.content_type || null,
        receiptSize: r.size || null,
        isEncrypted: !!r.is_encrypted,
        encryptedPayload: r.encrypted_payload || null,
        createdAt: new Date().toISOString(),
      });
    }
  }

  return c.text("OK");
});

// ==========================================
// Charities API (/api/charities)
// ==========================================

app.get("/api/charities", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);
  const list = await db.select().from(charities).where(eq(charities.userId, user.id)).all();
  return c.json({ charities: list });
});

app.post("/api/charities", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const newCharity = {
    id: body.id || crypto.randomUUID(),
    userId: user.id,
    name: body.name,
    ein: body.ein || null,
    category: body.category || null,
    status: body.status || null,
    classification: body.classification || null,
    nonprofitType: body.nonprofit_type || null,
    deductibility: body.deductibility || null,
    street: body.street || null,
    city: body.city || null,
    state: body.state || null,
    zip: body.zip || null,
    isEncrypted: !!body.is_encrypted,
    encryptedPayload: body.encrypted_payload || null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  await db.insert(charities).values(newCharity);
  await logAudit(db, user.id, "create", "charities", newCharity.id, `Created charity: ${body.name}`);

  return c.json({ id: newCharity.id }, 201);
});

app.put("/api/charities/:id", async (c) => {
  const user = c.get("user")!;
  const charityId = c.req.param("id");
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const existing = await db.select().from(charities).where(and(eq(charities.id, charityId), eq(charities.userId, user.id))).get();
  if (!existing) {
    return c.text("Not Found", 404);
  }

  const oldValues = JSON.stringify(existing);

  const updates: any = {
    updatedAt: new Date().toISOString(),
  };
  if (body.name !== undefined) updates.name = body.name;
  if (body.ein !== undefined) updates.ein = body.ein;
  if (body.category !== undefined) updates.category = body.category;
  if (body.status !== undefined) updates.status = body.status;
  if (body.classification !== undefined) updates.classification = body.classification;
  if (body.nonprofit_type !== undefined) updates.nonprofitType = body.nonprofit_type;
  if (body.deductibility !== undefined) updates.deductibility = body.deductibility;
  if (body.street !== undefined) updates.street = body.street;
  if (body.city !== undefined) updates.city = body.city;
  if (body.state !== undefined) updates.state = body.state;
  if (body.zip !== undefined) updates.zip = body.zip;
  if (body.is_encrypted !== undefined) updates.isEncrypted = body.is_encrypted;
  if (body.encrypted_payload !== undefined) updates.encryptedPayload = body.encrypted_payload;

  await db.update(charities).set(updates).where(eq(charities.id, charityId));

  const updated = await db.select().from(charities).where(eq(charities.id, charityId)).get();
  await logRevision(db, user.id, "charities", charityId, "update", oldValues, JSON.stringify(updated));
  await logAudit(db, user.id, "update", "charities", charityId, "Updated charity details");

  return c.json({ success: true });
});

app.delete("/api/charities/:id", async (c) => {
  const user = c.get("user")!;
  const charityId = c.req.param("id");
  const db = getDb(c.env.DB);

  const existing = await db.select().from(charities).where(and(eq(charities.id, charityId), eq(charities.userId, user.id))).get();
  if (!existing) {
    return c.text("Not Found", 404);
  }

  const oldValues = JSON.stringify(existing);

  // Check if charity is referenced by active donations
  const reference = await db.select().from(donations).where(and(eq(donations.charityId, charityId), eq(donations.deleted, false))).limit(1).get();
  if (reference) {
    return c.text("Cannot delete charity with active donations associated with it", 400);
  }

  await db.delete(charities).where(eq(charities.id, charityId));
  await logRevision(db, user.id, "charities", charityId, "delete", oldValues, null);
  await logAudit(db, user.id, "delete", "charities", charityId, `Deleted charity: ${existing.name}`);

  return c.json({ success: true });
});

// ProPublica Charity Search
app.get("/api/charities/search", async (c) => {
  const query = c.req.query("q") || "";
  if (!query) {
    return c.json([]);
  }

  const baseUrl = c.env.PROPUBLICA_API_BASE_URL || "https://projects.propublica.org/nonprofits/api/v2";
  const searchUrl = `${baseUrl}/search.json?q=${encodeURIComponent(query)}`;

  try {
    const res = await fetch(searchUrl, {
      headers: { "User-Agent": "DeductibleTracker/1.0" },
    });
    if (!res.ok) {
      throw new Error(`Upstream status ${res.status}`);
    }
    const data = (await res.json()) as any;
    const list = (data.organizations || []).map((org: any) => ({
      ein: org.strein || (org.ein ? org.ein.toString().padStart(9, "0") : ""),
      name: org.name,
      city: org.city || null,
      state: org.state || null,
    }));
    return c.json(list);
  } catch (err) {
    console.error("ProPublica search failed:", err);
    return c.json({ error: "Upstream API Error" }, 502);
  }
});

// ProPublica Charity Lookup by EIN
app.get("/api/charities/lookup/:ein", async (c) => {
  const ein = c.req.param("ein").replace(/\D/g, "");
  if (!ein) {
    return c.text("Invalid EIN", 400);
  }

  const baseUrl = c.env.PROPUBLICA_API_BASE_URL || "https://projects.propublica.org/nonprofits/api/v2";
  const lookupUrl = `${baseUrl}/organizations/${ein}.json`;

  try {
    const res = await fetch(lookupUrl, {
      headers: { "User-Agent": "DeductibleTracker/1.0" },
    });
    if (!res.ok) {
      return c.json(null);
    }
    const data = (await res.json()) as any;
    const org = data.organization;
    if (!org) return c.json(null);

    // Map fields
    return c.json({
      name: org.name,
      ein: org.strein || ein,
      category: org.ntee_code || null,
      status: org.exempt_organization_status_code ? "exempt" : "active",
      classification: org.foundation_code ? "Public Charity" : null,
      nonprofit_type: org.subsection_code ? `501(c)(${org.subsection_code})` : null,
      deductibility: org.deductibility_code ? "Deductible" : "Non-Deductible",
      street: org.address || null,
      city: org.city || null,
      state: org.state || null,
      zip: org.zipcode || null,
    });
  } catch (err) {
    console.error("ProPublica lookup failed:", err);
    return c.json(null);
  }
});

// ==========================================
// Receipts API (/api/receipts)
// ==========================================

app.get("/api/receipts", async (c) => {
  const user = c.get("user")!;
  const donationId = c.req.query("donation_id");
  const db = getDb(c.env.DB);

  let query = db
    .select({
      id: receipts.id,
      donation_id: receipts.donationId,
      key: receipts.receiptKey,
      file_name: receipts.fileName,
      content_type: receipts.contentType,
      size: receipts.receiptSize,
      ocr_text: receipts.ocrText,
      ocr_date: receipts.ocrDate,
      ocr_amount: receipts.ocrAmount,
      ocr_status: receipts.ocrStatus,
      is_encrypted: receipts.isEncrypted,
      encrypted_payload: receipts.encryptedPayload,
      created_at: receipts.createdAt,
    })
    .from(receipts)
    .innerJoin(donations, eq(donations.id, receipts.donationId))
    .where(eq(donations.userId, user.id));

  if (donationId) {
    query = query.where(eq(receipts.donationId, donationId));
  }

  const list = await query.all();
  return c.json({ receipts: list });
});

// Generate presigned upload URL
app.post("/api/receipts/upload", async (c) => {
  const user = c.get("user")!;
  const { file_type } = await c.req.json();
  const fileId = crypto.randomUUID();

  // Determine file extension
  let ext = "bin";
  if (file_type.includes("jpeg") || file_type.includes("jpg")) ext = "jpg";
  else if (file_type.includes("png")) ext = "png";
  else if (file_type.includes("pdf")) ext = "pdf";
  else if (file_type.includes("webp")) ext = "webp";

  const key = `receipts/${user.id}/${new Date().getFullYear()}/${fileId}.${ext}`;
  const expiresIn = 300;

  try {
    const uploadUrl = await presignUrl(
      "PUT",
      key,
      expiresIn,
      c.env as any
    );
    return c.json({
      upload_url: uploadUrl,
      key,
      expires_in: expiresIn,
    });
  } catch (err: any) {
    console.error("Presign upload error:", err);
    return c.json({ error: "Storage Presign Error" }, 500);
  }
});

// Generate presigned read URL
app.post("/api/receipts/presign", async (c) => {
  const user = c.get("user")!;
  const { key } = await c.req.json();

  const normalized = normalizeObjectKey(c.env.OBJECT_STORAGE_BUCKET || "", key);
  const prefix = userReceiptPrefix(user.id);

  if (!normalized.startsWith(prefix)) {
    return c.text("Forbidden", 403);
  }

  try {
    const downloadUrl = await presignUrl(
      "GET",
      normalized,
      300,
      c.env as any
    );
    return c.json({
      download_url: downloadUrl,
      key: normalized,
      expires_in: 300,
    });
  } catch (err: any) {
    console.error("Presign read error:", err);
    return c.json({ error: "Storage Presign Error" }, 500);
  }
});

// Confirm upload (save receipt details in DB)
app.post("/api/receipts/confirm", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);

  const donationId = body.donation_id;
  const key = normalizeObjectKey(c.env.OBJECT_STORAGE_BUCKET || "", body.key);
  const prefix = userReceiptPrefix(user.id);

  if (!key.startsWith(prefix)) {
    return c.text("Forbidden", 403);
  }

  // Check donation ownership
  const don = await db.select().from(donations).where(and(eq(donations.id, donationId), eq(donations.userId, user.id))).get();
  if (!don) {
    return c.text("Donation not found", 404);
  }

  const receiptId = crypto.randomUUID();
  await db.insert(receipts).values({
    id: receiptId,
    donationId,
    receiptKey: key,
    fileName: body.file_name || null,
    contentType: body.content_type || null,
    receiptSize: body.size || null,
    isEncrypted: !!body.is_encrypted,
    encryptedPayload: body.encrypted_payload || null,
    createdAt: new Date().toISOString(),
  });

  return c.json({ id: receiptId }, 201);
});

// Mistral OCR Receipt processing
app.post("/api/receipts/ocr", async (c) => {
  const user = c.get("user")!;
  const body = await c.req.json();
  const db = getDb(c.env.DB);
  const vaultKeyB64 = c.req.header("X-Vault-Key");

  let receiptId = body.id;
  let key = body.key;
  let contentType = body.content_type;
  let isEncrypted = false;
  let encryptedPayload = null;

  if (receiptId) {
    const rec = await db
      .select({
        id: receipts.id,
        key: receipts.receiptKey,
        contentType: receipts.contentType,
        isEncrypted: receipts.isEncrypted,
        encryptedPayload: receipts.encryptedPayload,
      })
      .from(receipts)
      .innerJoin(donations, eq(donations.id, receipts.donationId))
      .where(and(eq(receipts.id, receiptId), eq(donations.userId, user.id)))
      .get();
    if (!rec) {
      return c.text("Receipt not found", 404);
    }
    key = rec.key;
    contentType = rec.contentType;
    isEncrypted = !!rec.isEncrypted;
    encryptedPayload = rec.encryptedPayload;
  } else if (key) {
    key = normalizeObjectKey(c.env.OBJECT_STORAGE_BUCKET || "", key);
    const prefix = userReceiptPrefix(user.id);
    if (!key.startsWith(prefix)) {
      return c.text("Forbidden", 403);
    }
  } else {
    return c.text("Receipt id or key is required", 400);
  }

  const mistralKey = c.env.MISTRAL_API_KEY;
  if (!mistralKey) {
    return c.json({ status: "failed", warning: "OCR not configured on server" });
  }

  try {
    // 1. Get file bytes from OCI Object Storage
    const downloadUrl = await presignUrl(
      "GET",
      key,
      300,
      c.env as any
    );
    const resp = await fetch(downloadUrl);
    if (!resp.ok) {
      throw new Error(`Failed to download receipt: ${resp.status}`);
    }

    let fileBuffer = await resp.arrayBuffer();

    // 2. Decrypt transiently in server memory if encrypted
    if (isEncrypted && vaultKeyB64) {
      const rawKeyBytes = Uint8Array.from(atob(vaultKeyB64), (c) => c.charCodeAt(0));
      const keyObj = await crypto.subtle.importKey(
        "raw",
        rawKeyBytes,
        { name: "AES-GCM" },
        false,
        ["decrypt"]
      );

      const combined = new Uint8Array(fileBuffer);
      const iv = combined.slice(0, 12);
      const ciphertext = combined.slice(12);

      const decrypted = await crypto.subtle.decrypt(
        { name: "AES-GCM", iv },
        keyObj,
        ciphertext
      );
      fileBuffer = decrypted;
    }

    // 3. Prepare payload for Mistral OCR API
    const base64File = btoa(String.fromCharCode(...new Uint8Array(fileBuffer)));
    const mimeType = contentType || "application/octet-stream";
    const isImage = mimeType.startsWith("image/");

    const documentPayload = {
      type: isImage ? "image_url" : "document_url",
      [isImage ? "image_url" : "document_url"]: `data:${mimeType};base64,${base64File}`,
    };

    const ocrRequest = {
      model: c.env.MISTRAL_MODEL || "mistral-ocr-latest",
      document: documentPayload,
      include_image_base64: false,
      document_annotation_format: {
        type: "json_schema",
        json_schema: {
          name: "response_schema",
          schema: {
            type: "object",
            properties: {
              date_of_donation: {
                type: "string",
                format: "date",
                description: "Date of donation in ISO format (YYYY-MM-DD)",
              },
              organization_name: {
                type: ["string", "null"],
                description: "Name of the organization receiving the donation",
              },
              donation_type: {
                enum: ["money", "item"],
                description: "Type of donation - must be either 'money' or 'item'",
              },
              item_name: {
                type: ["string", "null"],
                description: "Name of the donated item (null for monetary donations)",
              },
              amount_usd: {
                type: ["number", "null"],
                description: "Amount donated in USD (null for non-monetary donations)",
              },
            },
            required: ["donation_type"],
            additional_properties: false,
          },
        },
      },
      document_annotation_prompt:
        "You are an expert document parser specializing in donation receipts.\nAnalyze the provided receipt text and extract key donation details.\nReturn structured JSON only. Do not include explanations.\nIf a field is missing, return null.\nNormalize dates into ISO format (YYYY-MM-DD).\nIf the donation is monetary, set item_name to null.\nIf the donation is an item, include item_name and set amount_usd to null unless explicitly stated.\nThe organization name should match the official entity on the receipt.\nClassify donation_type strictly as \"money\" or \"item\".",
    };

    const endpoint = c.env.MISTRAL_API_ENDPOINT || "https://api.mistral.ai/v1/ocr";
    const mistralResp = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${mistralKey}`,
      },
      body: JSON.stringify(ocrRequest),
    });

    if (!mistralResp.ok) {
      throw new Error(`Mistral status ${mistralResp.status}: ${await mistralResp.text()}`);
    }

    const ocrResult = (await mistralResp.json()) as any;
    const annotationVal = ocrResult.document_annotation;
    if (!annotationVal) {
      throw new Error("Mistral response missing document_annotation");
    }

    const suggestion = typeof annotationVal === "string" ? JSON.parse(annotationVal) : annotationVal;
    const ocrDate = suggestion.date_of_donation || null;
    const ocrAmount = suggestion.amount_usd || null;

    if (receiptId) {
      await db
        .update(receipts)
        .set({
          ocrDate,
          ocrAmount,
          ocrStatus: "done",
          ocrText: "Processed via Mistral OCR API",
          updatedAt: new Date().toISOString(),
        })
        .where(eq(receipts.id, receiptId));
      await logAudit(db, user.id, "ocr", "receipts", receiptId, "Processed receipt OCR");
    }

    return c.json({
      status: "done",
      id: receiptId || null,
      ocr_text: "Processed via Mistral OCR API",
      ocr_date: ocrDate,
      ocr_amount_usd: ocrAmount,
      suggestion: {
        date_of_donation: ocrDate,
        organization_name: suggestion.organization_name || null,
        donation_type: suggestion.donation_type,
        item_name: suggestion.item_name || null,
        amount_usd: ocrAmount,
      },
    });
  } catch (err: any) {
    console.error("OCR run failed:", err);
    return c.json({
      status: "failed",
      id: receiptId || null,
      warning: "Unable to parse receipt details automatically",
    });
  }
});

// ==========================================
// Valuations API (/api/valuations)
// ==========================================

app.post("/api/valuations/suggest", async (c) => {
  const { query } = await c.req.json();
  if (!query || !query.trim()) {
    return c.json([]);
  }

  const db = getDb(c.env.DB);
  const term = `%${query.trim().toLowerCase()}%`;

  const list = await db
    .select({
      name: valItems.name,
      min: valItems.suggestedMin,
      max: valItems.suggestedMax,
    })
    .from(valItems)
    .where(sql`LOWER(${valItems.name}) LIKE ${term}`)
    .limit(20)
    .all();

  return c.json(list.map(i => [i.name, i.min, i.max]));
});

app.post("/api/valuations/seed", async (c) => {
  const db = getDb(c.env.DB);

  // Check if items already exist
  const existing = await db.select().from(valItems).limit(1).get();
  if (existing) {
    return c.text("Valuations already seeded");
  }

  // Seed Categories
  const categories = [
    { id: "cat_appliances", name: "Appliances" },
    { id: "cat_childrens_clothing", name: "Children's Clothing" },
    { id: "cat_furniture", name: "Furniture" },
    { id: "cat_household_goods", name: "Household Goods" },
    { id: "cat_mens_clothing", name: "Men's Clothing" },
    { id: "cat_womens_clothing", name: "Women's Clothing" },
    { id: "cat_electronics", name: "Electronics & Computers" },
    { id: "cat_miscellaneous", name: "Miscellaneous" },
  ];

  for (const cat of categories) {
    await db.insert(valCategories).values(cat);
  }

  // Seed Items
  const items = [
    { id: "app_ac", categoryId: "cat_appliances", name: "Air Conditioner", suggestedMin: 21, suggestedMax: 93 },
    { id: "app_dryer", categoryId: "cat_appliances", name: "Dryer", suggestedMin: 47, suggestedMax: 93 },
    { id: "app_stove_elec", categoryId: "cat_appliances", name: "Electric Stove", suggestedMin: 78, suggestedMax: 156 },
    { id: "app_freezer", categoryId: "cat_appliances", name: "Freezer", suggestedMin: 25, suggestedMax: 100 },
    { id: "app_stove_gas", categoryId: "cat_appliances", name: "Gas Stove", suggestedMin: 52, suggestedMax: 130 },
    { id: "app_heater", categoryId: "cat_appliances", name: "Heater", suggestedMin: 8, suggestedMax: 23 },
    { id: "app_microwave", categoryId: "cat_appliances", name: "Microwave", suggestedMin: 10, suggestedMax: 50 },
    { id: "app_refrigerator", categoryId: "cat_appliances", name: "Refrigerator (Working)", suggestedMin: 78, suggestedMax: 259 },
    { id: "app_washer", categoryId: "cat_appliances", name: "Washing Machine", suggestedMin: 41, suggestedMax: 156 },
    { id: "app_coffeemaker", categoryId: "cat_appliances", name: "Coffee Maker", suggestedMin: 4, suggestedMax: 16 },
    { id: "app_iron", categoryId: "cat_appliances", name: "Iron", suggestedMin: 3, suggestedMax: 10 },
    
    { id: "child_blouse", categoryId: "cat_childrens_clothing", name: "Blouse", suggestedMin: 2, suggestedMax: 8 },
    { id: "child_boots", categoryId: "cat_childrens_clothing", name: "Boots", suggestedMin: 3, suggestedMax: 21 },
    { id: "child_coat", categoryId: "cat_childrens_clothing", name: "Coat", suggestedMin: 5, suggestedMax: 21 },
    { id: "child_dress", categoryId: "cat_childrens_clothing", name: "Dress", suggestedMin: 2, suggestedMax: 12 },
    { id: "child_jacket", categoryId: "cat_childrens_clothing", name: "Jacket", suggestedMin: 3, suggestedMax: 26 },
    { id: "child_jeans", categoryId: "cat_childrens_clothing", name: "Jeans", suggestedMin: 4, suggestedMax: 12 },
    { id: "child_pants", categoryId: "cat_childrens_clothing", name: "Pants", suggestedMin: 3, suggestedMax: 12 },
    { id: "child_shirt", categoryId: "cat_childrens_clothing", name: "Shirt", suggestedMin: 2, suggestedMax: 10 },
    { id: "child_shoes", categoryId: "cat_childrens_clothing", name: "Shoes", suggestedMin: 3, suggestedMax: 10 },
    { id: "child_snowsuit", categoryId: "cat_childrens_clothing", name: "Snowsuit", suggestedMin: 4, suggestedMax: 20 },
    { id: "child_sweater", categoryId: "cat_childrens_clothing", name: "Sweater", suggestedMin: 2, suggestedMax: 10 },

    { id: "furn_bed_full", categoryId: "cat_furniture", name: "Bed (full, queen, king)", suggestedMin: 52, suggestedMax: 176 },
    { id: "furn_bed_single", categoryId: "cat_furniture", name: "Bed (single)", suggestedMin: 36, suggestedMax: 104 },
    { id: "furn_chair_uph", categoryId: "cat_furniture", name: "Chair (upholstered)", suggestedMin: 26, suggestedMax: 104 },
    { id: "furn_chest", categoryId: "cat_furniture", name: "Chest", suggestedMin: 26, suggestedMax: 99 },
    { id: "furn_china", categoryId: "cat_furniture", name: "China Cabinet", suggestedMin: 89, suggestedMax: 311 },
    { id: "furn_coffee_table", categoryId: "cat_furniture", name: "Coffee Table", suggestedMin: 15, suggestedMax: 100 },
    { id: "furn_desk", categoryId: "cat_furniture", name: "Desk", suggestedMin: 26, suggestedMax: 145 },
    { id: "furn_dresser", categoryId: "cat_furniture", name: "Dresser", suggestedMin: 20, suggestedMax: 104 },
    { id: "furn_end_table", categoryId: "cat_furniture", name: "End Table", suggestedMin: 10, suggestedMax: 75 },
    { id: "furn_kitchen_set", categoryId: "cat_furniture", name: "Kitchen Set", suggestedMin: 35, suggestedMax: 176 },
    { id: "furn_sofa", categoryId: "cat_furniture", name: "Sofa", suggestedMin: 36, suggestedMax: 395 },

    { id: "house_blanket", categoryId: "cat_household_goods", name: "Blanket", suggestedMin: 3, suggestedMax: 16 },
    { id: "house_curtains", categoryId: "cat_household_goods", name: "Curtains", suggestedMin: 2, suggestedMax: 12 },
    { id: "house_lamp_floor", categoryId: "cat_household_goods", name: "Lamp, Floor", suggestedMin: 6, suggestedMax: 52 },
    { id: "house_lamp_table", categoryId: "cat_household_goods", name: "Lamp, Table", suggestedMin: 3, suggestedMax: 20 },
    { id: "house_pillow", categoryId: "cat_household_goods", name: "Pillow", suggestedMin: 2, suggestedMax: 8 },
    { id: "house_rug_area", categoryId: "cat_household_goods", name: "Area Rug", suggestedMin: 2, suggestedMax: 93 },
    { id: "house_sheets", categoryId: "cat_household_goods", name: "Sheets", suggestedMin: 2, suggestedMax: 9 },

    { id: "men_jacket", categoryId: "cat_mens_clothing", name: "Jacket", suggestedMin: 8, suggestedMax: 45 },
    { id: "men_suit", categoryId: "cat_mens_clothing", name: "Suit (2pc)", suggestedMin: 5, suggestedMax: 96 },
    { id: "men_shirt", categoryId: "cat_mens_clothing", name: "Shirt", suggestedMin: 3, suggestedMax: 12 },
    { id: "men_pants", categoryId: "cat_mens_clothing", name: "Pants", suggestedMin: 4, suggestedMax: 23 },
    { id: "men_shoes", categoryId: "cat_mens_clothing", name: "Shoes", suggestedMin: 3, suggestedMax: 30 },
    { id: "men_sweater", categoryId: "cat_mens_clothing", name: "Sweater", suggestedMin: 3, suggestedMax: 12 },

    { id: "women_suit", categoryId: "cat_womens_clothing", name: "Suit (2pc)", suggestedMin: 10, suggestedMax: 96 },
    { id: "women_blouse", categoryId: "cat_womens_clothing", name: "Blouse", suggestedMin: 3, suggestedMax: 12 },
    { id: "women_dress", categoryId: "cat_womens_clothing", name: "Dress", suggestedMin: 4, suggestedMax: 28 },
    { id: "women_pants", categoryId: "cat_womens_clothing", name: "Pants", suggestedMin: 4, suggestedMax: 23 },
    { id: "women_shoes", categoryId: "cat_womens_clothing", name: "Shoes", suggestedMin: 2, suggestedMax: 30 },
    { id: "women_sweater", categoryId: "cat_womens_clothing", name: "Sweater", suggestedMin: 4, suggestedMax: 13 },

    { id: "elec_desktop", categoryId: "cat_electronics", name: "Desktop Computer", suggestedMin: 20, suggestedMax: 415 },
    { id: "elec_laptop", categoryId: "cat_electronics", name: "Laptop", suggestedMin: 25, suggestedMax: 415 },
    { id: "elec_monitor", categoryId: "cat_electronics", name: "Monitor", suggestedMin: 5, suggestedMax: 51 },
    { id: "elec_printer", categoryId: "cat_electronics", name: "Printer", suggestedMin: 1, suggestedMax: 155 },
    { id: "elec_tablet", categoryId: "cat_electronics", name: "Tablet", suggestedMin: 25, suggestedMax: 150 },
    { id: "elec_tv", categoryId: "cat_electronics", name: "TV (Color Working)", suggestedMin: 78, suggestedMax: 233 },

    { id: "misc_bicycle", categoryId: "cat_miscellaneous", name: "Bicycle", suggestedMin: 5, suggestedMax: 83 },
    { id: "misc_books_hard", categoryId: "cat_miscellaneous", name: "Book (hardback)", suggestedMin: 1, suggestedMax: 3 },
    { id: "misc_books_paper", categoryId: "cat_miscellaneous", name: "Book (paperback)", suggestedMin: 1, suggestedMax: 2 },
    { id: "misc_luggage", categoryId: "cat_miscellaneous", name: "Luggage", suggestedMin: 5, suggestedMax: 16 },
    { id: "misc_vacuum", categoryId: "cat_miscellaneous", name: "Vacuum Cleaner", suggestedMin: 5, suggestedMax: 67 },
  ];

  for (const item of items) {
    await db.insert(valItems).values(item);
  }

  return c.text("Seed complete");
});

app.get("/api/valuations/tree", async (c) => {
  const db = getDb(c.env.DB);
  const categoriesList = await db.select().from(valCategories).all();
  const itemsList = await db.select().from(valItems).all();

  const tree = categoriesList.map((cat) => {
    return {
      id: cat.id,
      name: cat.name,
      items: itemsList
        .filter((i) => i.categoryId === cat.id)
        .map((i) => ({
          name: i.name,
          min: i.suggestedMin,
          max: i.suggestedMax,
        })),
    };
  });

  return c.json(tree);
});

// ==========================================
// Reports & Exports API (/api/reports)
// ==========================================

app.get("/api/reports/years", async (c) => {
  const user = c.get("user")!;
  const db = getDb(c.env.DB);

  const res = await db
    .select({ year: donations.donationYear })
    .from(donations)
    .where(and(eq(donations.userId, user.id), eq(donations.deleted, false)))
    .groupBy(donations.donationYear)
    .orderBy(desc(donations.donationYear))
    .all();

  return c.json({ years: res.map((r) => r.year).filter(Boolean) as number[] });
});

function csvEscape(s: string): string {
  let norm = s;
  if (norm.startsWith("=") || norm.startsWith("+") || norm.startsWith("-") || norm.startsWith("@")) {
    norm = `'${norm}`;
  }
  if (norm.includes(",") || norm.includes('"') || norm.includes("\n")) {
    return `"${norm.replace(/"/g, '""')}"`;
  }
  return norm;
}

app.get("/api/reports/export", async (c) => {
  const user = c.get("user")!;
  const yearQuery = c.req.query("year");
  const db = getDb(c.env.DB);

  let query = db
    .select({
      id: donations.id,
      date: donations.donationDate,
      category: donations.donationCategory,
      amount: donations.donationAmount,
      notes: donations.notes,
      charityName: charities.name,
      charityId: charities.id,
    })
    .from(donations)
    .innerJoin(charities, eq(charities.id, donations.charityId))
    .where(and(eq(donations.userId, user.id), eq(donations.deleted, false)));

  if (yearQuery) {
    query = query.where(eq(donations.donationYear, parseInt(yearQuery, 10)));
  }

  const list = await query.all();

  let csv = "id,date,category,amount,charity_name,charity_id,notes\n";
  for (const d of list) {
    csv += `${csvEscape(d.id)},${csvEscape(d.date || "")},${csvEscape(d.category || "")},${csvEscape(
      d.amount?.toFixed(2) || "0.00"
    )},${csvEscape(d.charityName)},${csvEscape(d.charityId)},${csvEscape(d.notes || "")}\n`;
  }

  c.res.headers.set("Content-Type", "text/csv; charset=utf-8");
  c.res.headers.set("Content-Disposition", "attachment; filename=donations.csv");
  return c.text(csv);
});

app.get("/api/reports/export/txf", async (c) => {
  const user = c.get("user")!;
  const yearQuery = c.req.query("year");
  const db = getDb(c.env.DB);

  let query = db
    .select({
      id: donations.id,
      date: donations.donationDate,
      amount: donations.donationAmount,
      notes: donations.notes,
      charityName: charities.name,
      charityEin: charities.ein,
    })
    .from(donations)
    .innerJoin(charities, eq(charities.id, donations.charityId))
    .where(and(eq(donations.userId, user.id), eq(donations.deleted, false)));

  if (yearQuery) {
    query = query.where(eq(donations.donationYear, parseInt(yearQuery, 10)));
  }

  const list = await query.all();

  let txf = "V042\nADeductible Tracker\n";
  txf += `D${new Date().toLocaleDateString("en-US")}\n^\n`;

  for (const d of list) {
    const memo = `Donation ID: ${d.id}${d.charityEin ? ` | EIN: ${d.charityEin}` : ""}${
      d.notes ? ` | Notes: ${d.notes}` : ""
    }`.replace(/[\^\r\n]/g, " ");

    txf += "TD\nN323\nC1\nLCharitable contributions\n";
    txf += `P${d.charityName.replace(/[\^\r\n]/g, " ")}\n`;
    txf += `D${(d.date || "").replace(/[\^\r\n]/g, " ")}\n`;
    txf += `\$${(d.amount || 0).toFixed(2)}\n`;
    txf += `M${memo}\n^\n`;
  }

  c.res.headers.set("Content-Type", "application/octet-stream");
  c.res.headers.set("Content-Disposition", "attachment; filename=donations-tax-export.txf");
  return c.body(txf);
});

app.get("/api/reports/audit", async (c) => {
  const user = c.get("user")!;
  const since = c.req.query("since");
  const db = getDb(c.env.DB);

  let query = db.select().from(auditLogs).where(eq(auditLogs.userId, user.id));
  if (since) {
    query = query.where(sql`${auditLogs.createdAt} > ${since}`);
  }

  const list = await query.all();

  let csv = "id,user_id,action,table_name,record_id,details,created_at\n";
  for (const a of list) {
    csv += `${csvEscape(a.id)},${csvEscape(a.userId)},${csvEscape(a.action)},${csvEscape(a.tableName)},${csvEscape(
      a.recordId || ""
    )},${csvEscape(a.details || "")},${csvEscape(a.createdAt || "")}\n`;
  }

  c.res.headers.set("Content-Type", "text/csv; charset=utf-8");
  c.res.headers.set("Content-Disposition", "attachment; filename=audit_logs.csv");
  return c.text(csv);
});

// ==========================================
// Tax API (/api/tax)
// ==========================================

const filingStatuses = ["single", "married_joint", "married_separate", "head_household"] as const;

const SINGLE_BRACKETS = [
  { rate: 0.10, min: 0.0, max: 11925.0 },
  { rate: 0.12, min: 11925.0, max: 48475.0 },
  { rate: 0.22, min: 48475.0, max: 103350.0 },
  { rate: 0.24, min: 103350.0, max: 197300.0 },
  { rate: 0.32, min: 197300.0, max: 250525.0 },
  { rate: 0.35, min: 250525.0, max: 626350.0 },
  { rate: 0.37, min: 626350.0, max: null },
];

const JOINT_BRACKETS = [
  { rate: 0.10, min: 0.0, max: 23850.0 },
  { rate: 0.12, min: 23850.0, max: 96950.0 },
  { rate: 0.22, min: 96950.0, max: 206700.0 },
  { rate: 0.24, min: 206700.0, max: 394600.0 },
  { rate: 0.32, min: 394600.0, max: 501050.0 },
  { rate: 0.35, min: 501050.0, max: 751600.0 },
  { rate: 0.37, min: 751600.0, max: null },
];

const SEPARATE_BRACKETS = [
  { rate: 0.10, min: 0.0, max: 11925.0 },
  { rate: 0.12, min: 11925.0, max: 48475.0 },
  { rate: 0.22, min: 48475.0, max: 103350.0 },
  { rate: 0.24, min: 103350.0, max: 197300.0 },
  { rate: 0.32, min: 197300.0, max: 250525.0 },
  { rate: 0.35, min: 250525.0, max: 375800.0 },
  { rate: 0.37, min: 375800.0, max: null },
];

const HEAD_BRACKETS = [
  { rate: 0.10, min: 0.0, max: 17000.0 },
  { rate: 0.12, min: 17000.0, max: 64850.0 },
  { rate: 0.22, min: 64850.0, max: 103350.0 },
  { rate: 0.24, min: 103350.0, max: 197300.0 },
  { rate: 0.32, min: 197300.0, max: 250500.0 },
  { rate: 0.35, min: 250500.0, max: 626350.0 },
  { rate: 0.37, min: 626350.0, max: null },
];

app.get("/api/tax/marginal-rate", (c) => {
  const status = c.req.query("filing_status") || "single";
  const agiVal = c.req.query("agi");

  let brackets = SINGLE_BRACKETS;
  if (status === "married_joint") brackets = JOINT_BRACKETS;
  else if (status === "married_separate") brackets = SEPARATE_BRACKETS;
  else if (status === "head_household") brackets = HEAD_BRACKETS;

  const agi = agiVal ? parseFloat(agiVal) : null;
  let selectedRate = null;

  if (agi !== null && !isNaN(agi) && agi >= 0) {
    for (const b of brackets) {
      if (agi >= b.min && (b.max === null || agi <= b.max)) {
        selectedRate = b.rate;
        break;
      }
    }
  }

  return c.json({
    filing_status: status,
    agi,
    selected_rate: selectedRate,
    brackets,
  });
});

export default app;
