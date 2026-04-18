/**
 * Authentication state management.
 *
 * Owns the cached auth flag, TTL-gated /api/me checks, user
 * profile hydration, and logout flow.
 */

import { apiJson } from './http.js';
import {
  clearCurrentUser,
  getCurrentUserId,
  setCurrentUser,
} from './current-user.js';

// Auth state (cached to avoid extra requests on every click)
let authenticated = false;
let returnTo = null;
let lastAuthCheckAt = 0;
let lastAuthResult = false;
let authCheckInFlight = null;
let lastUserFetchAt = 0;

export function isAuthenticated() {
  return authenticated;
}

export function setAuthenticated(value) {
  authenticated = value;
}

export function getReturnTo() {
  return returnTo;
}

export function setReturnTo(value) {
  returnTo = value;
}

export function consumeReturnTo() {
  const value = returnTo;
  returnTo = null;
  return value;
}

export async function checkAuthCached() {
  const now = Date.now();
  const ttlMs = 4000;
  if (authCheckInFlight) return authCheckInFlight;
  if (now - lastAuthCheckAt < ttlMs) return lastAuthResult;
  authCheckInFlight = (async () => {
    try {
      const res = await fetch('/api/me', { credentials: 'include' });
      // Treat 429 as "unknown" and keep the last known result
      if (res.status === 429) return lastAuthResult;

      if (!res.ok && res.status !== 401) {
        console.error('Auth check failed', res.status);
      }

      lastAuthResult = res.ok;
      lastAuthCheckAt = Date.now();
      if (res.ok) {
        const shouldFetchUser = now - lastUserFetchAt > ttlMs || !getCurrentUserId();
        if (shouldFetchUser) {
          try {
            const profile = await res.json();
            setCurrentUser(profile);
            lastUserFetchAt = Date.now();
          } catch (e) {
            /* ignore */
          }
        }
      } else {
        clearCurrentUser();
      }
      return lastAuthResult;
    } catch (e) {
      return lastAuthResult;
    } finally {
      authCheckInFlight = null;
    }
  })();
  return authCheckInFlight;
}

export function resetAuthTimers() {
  lastAuthResult = false;
  lastAuthCheckAt = 0;
  lastUserFetchAt = 0;
}

export async function handleLogout(deps) {
  const { clearUserCaches, renderLogin } = deps;

  try {
    await apiJson('/auth/logout', { method: 'POST' });
  } catch (e) {
    console.warn('Logout request failed', e);
  }

  const nav = document.getElementById('nav-container');
  if (nav) nav.classList.add('hidden');
  const authActions = document.getElementById('auth-actions');
  if (authActions) authActions.classList.add('hidden');

  authenticated = false;

  const profileKey = getProfileStorageKey();
  await clearUserCaches();
  try {
    localStorage.removeItem(profileKey);
  } catch (e) {
    /* ignore */
  }
  clearCurrentUser();
  resetAuthTimers();

  renderLogin();

  try {
    window.location.replace('/');
  } catch (e) {
    /* ignore */
  }
}

function getProfileStorageKey() {
  const userId = getCurrentUserId();
  return userId ? `profile:${userId}` : 'profile:anonymous';
}
