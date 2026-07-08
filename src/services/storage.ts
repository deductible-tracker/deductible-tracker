import { normalize } from "path";

export function normalizeObjectKey(bucketName: string, key: string): string {
  const trimmed = key.trim().replace(/^\/+/, "");
  const bucketPrefix = `${bucketName}/`;
  
  const normalized = trimmed.startsWith(bucketPrefix)
    ? trimmed.substring(bucketPrefix.length)
    : trimmed;

  if (normalized.includes("..") || normalized.includes("\0")) {
    return "";
  }
  return normalized;
}

export function userReceiptPrefix(userId: string): string {
  return `receipts/${userId}/`;
}

// Simple pure JS/TS AWS Signature V4 presign helper using Web Crypto API (supported in Cloudflare Workers)
async function hmacSha256(key: ArrayBuffer | Uint8Array, data: string | Uint8Array): Promise<ArrayBuffer> {
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const dataBytes = typeof data === "string" ? new TextEncoder().encode(data) : data;
  return await crypto.subtle.sign("HMAC", cryptoKey, dataBytes);
}

async function sha256(data: string): Promise<ArrayBuffer> {
  const dataBytes = new TextEncoder().encode(data);
  return await crypto.subtle.digest("SHA-256", dataBytes);
}

function bufToHex(buf: ArrayBuffer): string {
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function presignUrl(
  method: string,
  key: string,
  expiresInSecs: number,
  env: {
    OBJECT_STORAGE_ENDPOINT: string;
    OBJECT_STORAGE_BUCKET: string;
    OCI_REGION: string;
    OCI_ACCESS_KEY_ID: string;
    OCI_SECRET_ACCESS_KEY: string;
  }
): Promise<string> {
  const normalizedKey = normalizeObjectKey(env.OBJECT_STORAGE_BUCKET, key);
  if (!normalizedKey) {
    throw new Error("Storage key cannot be empty");
  }

  const endpointUrl = new URL(env.OBJECT_STORAGE_ENDPOINT);
  const host = endpointUrl.port
    ? `${endpointUrl.hostname}:${endpointUrl.port}`
    : endpointUrl.hostname;

  const basePath = endpointUrl.pathname.replace(/^\/+|\/+$/g, "");
  const objectPath = basePath
    ? `${basePath}/${env.OBJECT_STORAGE_BUCKET}/${normalizedKey}`
    : `${env.OBJECT_STORAGE_BUCKET}/${normalizedKey}`;

  const canonicalUri = "/" + objectPath.split("/").map(encodeURIComponent).join("/");

  const now = new Date();
  const amzDate = now.toISOString().replace(/[:-]/g, "").split(".")[0] + "Z";
  const dateStamp = amzDate.substring(0, 8);

  const credentialScope = `${dateStamp}/${env.OCI_REGION}/s3/aws4_request`;
  const credential = `${env.OCI_ACCESS_KEY_ID}/${credentialScope}`;

  const queryParams: Record<string, string> = {
    "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
    "X-Amz-Credential": credential,
    "X-Amz-Date": amzDate,
    "X-Amz-Expires": expiresInSecs.toString(),
    "X-Amz-SignedHeaders": "host",
  };

  const sortedKeys = Object.keys(queryParams).sort();
  const canonicalQuery = sortedKeys
    .map((k) => `${encodeURIComponent(k)}=${encodeURIComponent(queryParams[k])}`)
    .join("&");

  const canonicalHeaders = `host:${host}\n`;
  const canonicalRequest = `${method}\n${canonicalUri}\n${canonicalQuery}\n${canonicalHeaders}\nhost\nUNSIGNED-PAYLOAD`;

  const canonicalRequestHash = bufToHex(await sha256(canonicalRequest));
  const stringToSign = `AWS4-HMAC-SHA256\n${amzDate}\n${credentialScope}\n${canonicalRequestHash}`;

  // Calculate signature
  const kSecret = new TextEncoder().encode("AWS4" + env.OCI_SECRET_ACCESS_KEY);
  const kDate = new Uint8Array(await hmacSha256(kSecret, dateStamp));
  const kRegion = new Uint8Array(await hmacSha256(kDate, env.OCI_REGION));
  const kService = new Uint8Array(await hmacSha256(kRegion, "s3"));
  const kSigning = new Uint8Array(await hmacSha256(kService, "aws4_request"));
  const signature = bufToHex(await hmacSha256(kSigning, stringToSign));

  const finalUrl = new URL(env.OBJECT_STORAGE_ENDPOINT);
  finalUrl.pathname = objectPath;
  finalUrl.search = `${canonicalQuery}&X-Amz-Signature=${signature}`;

  return finalUrl.toString();
}
