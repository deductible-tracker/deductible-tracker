import { getCurrentUser } from "./currentUser";

const VAULT_CHALLENGE = "Deductible Tracker Vault Challenge - Do Not Share";
let cachedVaultKey: CryptoKey | null = null;

export async function ensureVaultKey(userId: string): Promise<CryptoKey | null> {
  if (cachedVaultKey) return cachedVaultKey;
  const user = getCurrentUser();
  if (user && user.is_encrypted) {
    if (user.vault_credential_id) {
      cachedVaultKey = await unlockVaultKey(userId, user.vault_credential_id);
    } else {
      console.warn("Vault enabled but no credential ID found. Re-registration may be required.");
      const result = await registerVaultKey(userId);
      cachedVaultKey = result.key;
    }
  }
  return cachedVaultKey;
}

export async function registerVaultKey(userId: string): Promise<{ key: CryptoKey; credentialId: string }> {
  if (!(window as any).PublicKeyCredential) {
    throw new Error("WebAuthn is not supported in this browser.");
  }

  const challenge = new TextEncoder().encode(VAULT_CHALLENGE);
  const userBuffer = new TextEncoder().encode(userId);

  const options: PublicKeyCredentialCreationOptions = {
    challenge,
    rp: { name: "Deductible Tracker" },
    user: {
      id: userBuffer,
      name: userId,
      displayName: userId,
    },
    pubKeyCredParams: [
      { alg: -7, type: "public-key" }, // ES256
      { alg: -257, type: "public-key" } // RS256
    ],
    timeout: 60000,
    attestation: "none"
  };

  const credential = await navigator.credentials.create({ publicKey: options }) as any;
  if (!credential) throw new Error("Failed to create Passkey credential");

  const key = await deriveKeyFromRawId(credential.rawId);
  const credentialId = btoa(String.fromCharCode(...new Uint8Array(credential.rawId)));

  return { key, credentialId };
}

export async function unlockVaultKey(userId: string, credentialIdB64: string): Promise<CryptoKey> {
  if (!(window as any).PublicKeyCredential) {
    throw new Error("WebAuthn is not supported in this browser.");
  }

  const challenge = new TextEncoder().encode(VAULT_CHALLENGE);
  const credentialId = Uint8Array.from(atob(credentialIdB64), (c) => c.charCodeAt(0));

  const options: PublicKeyCredentialRequestOptions = {
    challenge,
    allowCredentials: [{
      id: credentialId,
      type: "public-key"
    }],
    timeout: 60000,
    userVerification: "required"
  };

  const assertion = await navigator.credentials.get({ publicKey: options }) as any;
  if (!assertion) throw new Error("Failed to unlock with Passkey");

  return await deriveKeyFromRawId(assertion.rawId);
}

async function deriveKeyFromRawId(rawId: ArrayBuffer): Promise<CryptoKey> {
  const hash = await crypto.subtle.digest("SHA-256", rawId);
  return await crypto.subtle.importKey(
    "raw",
    hash,
    { name: "AES-GCM" },
    false,
    ["encrypt", "decrypt"]
  );
}

export async function encryptData(key: CryptoKey, data: any): Promise<string> {
  const encoded = new TextEncoder().encode(JSON.stringify(data));
  return encryptBinaryData(key, encoded);
}

export async function encryptBinaryData(key: CryptoKey, bytes: ArrayBuffer | Uint8Array): Promise<string> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    bytes
  );

  const combined = new Uint8Array(iv.length + ciphertext.byteLength);
  combined.set(iv, 0);
  combined.set(new Uint8Array(ciphertext), iv.length);
  
  return btoa(String.fromCharCode(...combined));
}

export async function decryptData(key: CryptoKey, base64Data: string): Promise<any> {
  const combined = Uint8Array.from(atob(base64Data), (c) => c.charCodeAt(0));
  const iv = combined.slice(0, 12);
  const ciphertext = combined.slice(12);

  const decrypted = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    key,
    ciphertext
  );

  return JSON.parse(new TextDecoder().decode(decrypted));
}

export async function decryptBinaryData(key: CryptoKey, combinedBytes: Uint8Array): Promise<ArrayBuffer> {
  const iv = combinedBytes.slice(0, 12);
  const ciphertext = combinedBytes.slice(12);
  return await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    key,
    ciphertext
  );
}

export function isWebAuthnSupported(): boolean {
  return !!((window as any).PublicKeyCredential && 
            PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable && 
            PublicKeyCredential.isConditionalMediationAvailable);
}

