// static/js/services/crypto.js
import { getCurrentUser } from './current-user.js';

const VAULT_CHALLENGE = "Deductible Tracker Vault Challenge - Do Not Share";
let cachedVaultKey = null;

export async function ensureVaultKey(userId) {
  if (cachedVaultKey) return cachedVaultKey;
  const user = getCurrentUser();
  if (user && user.is_encrypted) {
    if (user.vault_credential_id) {
      cachedVaultKey = await unlockVaultKey(userId, user.vault_credential_id);
    } else {
      // Fallback for transition - if no ID stored, we must re-register or fail
      console.warn('Vault enabled but no credential ID found. Re-registration may be required.');
      const result = await registerVaultKey(userId);
      cachedVaultKey = result.key;
    }
  }
  return cachedVaultKey;
}

/**
 * Registers a NEW Passkey for the vault.
 * Returns { key, credentialId }
 */
export async function registerVaultKey(userId) {
  if (!window.PublicKeyCredential) {
    throw new Error('WebAuthn is not supported in this browser.');
  }

  const challenge = new TextEncoder().encode(VAULT_CHALLENGE);
  const userBuffer = new TextEncoder().encode(userId);

  const options = {
    publicKey: {
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
    }
  };

  const credential = await navigator.credentials.create(options);
  if (!credential) throw new Error('Failed to create Passkey credential');

  // Derive key from rawId (consistent for this credential)
  const key = await deriveKeyFromRawId(credential.rawId);
  const credentialId = btoa(String.fromCharCode(...new Uint8Array(credential.rawId)));

  return { key, credentialId };
}

/**
 * Unlocks the vault using an EXISTING Passkey.
 */
export async function unlockVaultKey(userId, credentialIdB64) {
  if (!window.PublicKeyCredential) {
    throw new Error('WebAuthn is not supported in this browser.');
  }

  const challenge = new TextEncoder().encode(VAULT_CHALLENGE);
  const credentialId = Uint8Array.from(atob(credentialIdB64), c => c.charCodeAt(0));

  const options = {
    publicKey: {
      challenge,
      allowCredentials: [{
        id: credentialId,
        type: 'public-key'
      }],
      timeout: 60000,
      userVerification: 'required'
    }
  };

  const assertion = await navigator.credentials.get(options);
  if (!assertion) throw new Error('Failed to unlock with Passkey');

  // We use the rawId of the selected credential to derive the key
  return await deriveKeyFromRawId(assertion.rawId);
}

async function deriveKeyFromRawId(rawId) {
  const hash = await crypto.subtle.digest('SHA-256', rawId);
  return await crypto.subtle.importKey(
    'raw',
    hash,
    { name: 'AES-GCM' },
    false,
    ['encrypt', 'decrypt']
  );
}

export async function encryptData(key, data) {
  const encoded = new TextEncoder().encode(JSON.stringify(data));
  return encryptBinaryData(key, encoded);
}

export async function encryptBinaryData(key, bytes) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  
  const ciphertext = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    key,
    bytes
  );

  // Return base64 combined IV + Ciphertext
  const combined = new Uint8Array(iv.length + ciphertext.byteLength);
  combined.set(iv, 0);
  combined.set(new Uint8Array(ciphertext), iv.length);
  
  return btoa(String.fromCharCode(...combined));
}

export async function decryptData(key, base64Data) {
  const combined = new Uint8Array(atob(base64Data).split('').map(c => c.charCodeAt(0)));
  const iv = combined.slice(0, 12);
  const ciphertext = combined.slice(12);

  const decrypted = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv },
    key,
    ciphertext
  );

  return JSON.parse(new TextDecoder().decode(decrypted));
}

export function isWebAuthnSupported() {
  return !!(window.PublicKeyCredential && 
            PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable && 
            PublicKeyCredential.isConditionalMediationAvailable);
}
