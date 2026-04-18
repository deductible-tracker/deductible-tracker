/**
 * Login view — renders the sign-in page (Google + optional dev login)
 * and handles post-login bootstrapping.
 */

import { apiJson } from '../services/http.js';
import { getCurrentUserId, setCurrentUser } from '../services/current-user.js';
import {
  setAuthenticated,
  consumeReturnTo,
} from '../services/auth-state.js';

let googleIdentityInitKey = null;

/**
 * Render the login page and wire form + Google Identity.
 * @param {object} deps  Dependency bag from the app init
 */
export async function renderLoginView(deps) {
  const {
    clearUserCaches,
    navigate,
    refreshCharitiesCache,
    refreshDonationsFromServer,
    restoreAppShellIfNeeded,
    Sync,
    updateTotals,
  } = deps;

  const app = document.getElementById('app');
  app.innerHTML = `
        <div class="mx-auto grid min-h-full max-w-6xl items-center gap-6 py-4 sm:gap-8 sm:py-12 lg:grid-cols-2">
            <div class="dt-panel p-6 sm:p-8 order-1 lg:order-2">
                <h2 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Sign in</h2>
                <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Continue with Google to create or access your account.</p>
                
                <div class="mt-8 flex flex-col items-center justify-center space-y-6">
                  <div id="g_id_onload" data-context="signin" data-ux_mode="redirect" data-auto_prompt="false"></div>

                  <div class="flex min-h-11 w-full max-w-[320px] items-center justify-center rounded-xl border border-slate-200 bg-white/85 px-3 py-1 dark:border-slate-700 dark:bg-slate-800/85">
                    <div class="g_id_signin"
                      data-type="standard"
                      data-shape="rectangular"
                      data-theme="outline"
                      data-text="signin_with"
                      data-size="large"
                      data-logo_alignment="left">
                    </div>
                  </div>

                    <div id="traditional-login-section" class="hidden w-full space-y-6">
                        <div class="relative w-full">
                            <div class="absolute inset-0 flex items-center" aria-hidden="true">
                                <div class="w-full border-t border-slate-200 dark:border-slate-700"></div>
                            </div>
                            <div class="relative flex justify-center text-sm">
                                <span class="bg-white px-2 text-slate-500 dark:bg-slate-800">Or use traditional login</span>
                            </div>
                        </div>

                        <form class="w-full space-y-4" id="login-form">
                            <div>
                                <label for="username" class="dt-label">Username</label>
                                <input id="username" name="username" type="text" required autocomplete="username" class="dt-input" placeholder="Username" />
                            </div>
                            <div>
                                <label for="password" class="dt-label">Password</label>
                                <input id="password" name="password" type="password" required autocomplete="current-password" class="dt-input" placeholder="Password" />
                            </div>
                            <div class="pt-2">
                                <button type="submit" class="dt-btn-primary w-full">
                                    Sign in
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
            <div class="rounded-2xl border border-slate-200 dark:border-slate-700 bg-linear-to-br from-indigo-50 to-white p-6 sm:p-10 order-2 lg:order-1">
                <p class="text-xs font-semibold uppercase tracking-[0.14em] text-indigo-600 dark:text-indigo-400">Deductible Tracker</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-slate-900 dark:text-slate-100 sm:text-4xl">A better way to track charitable giving</h1>
                <p class="mt-4 max-w-xl text-sm text-slate-600 dark:text-slate-300 sm:text-base">An offline-first replacement for Turbotax's It's Deductible.</p>
                <div class="mt-6 grid gap-3 text-sm text-slate-600 dark:text-slate-300 sm:grid-cols-2">
                    <div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3">Fast donation entry</div>
                    <div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3">Receipt management</div>
                    <div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3">Offline-first sync</div>
                    <div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3">CSV exports</div>
                </div>
            </div>
        </div>
    `;

  // Fetch config to see if dev login is allowed and whether Google Sign-In is configured.
  try {
    const configRes = await fetch('/api/config');
    if (configRes.ok) {
      const config = await configRes.json();
      if (config.allow_dev_login) {
        document.getElementById('traditional-login-section').classList.remove('hidden');
      }

      const gIdOnload = document.getElementById('g_id_onload');
      const gIdSignin = document.querySelector('.g_id_signin');
      if (config.google_enabled && config.google_client_id) {
        try {
          gIdOnload.setAttribute('data-client_id', config.google_client_id);
          if (config.oauth_state) {
            gIdOnload.setAttribute('data-state', config.oauth_state);
          }
          if (!gIdOnload.getAttribute('data-login_uri')) {
            gIdOnload.setAttribute('data-login_uri', `${location.origin}/auth/callback/google`);
          }

          if (!window.google || !google.accounts || !google.accounts.id) {
            await new Promise((resolve, reject) => {
              const s = document.createElement('script');
              s.src = 'https://accounts.google.com/gsi/client';
              s.async = true;
              s.defer = true;
              s.onload = resolve;
              s.onerror = reject;
              document.head.appendChild(s);
            });
          }

          if (window.google && google.accounts && google.accounts.id && gIdSignin) {
            const clientId = gIdOnload.getAttribute('data-client_id');
            const loginUri = gIdOnload.getAttribute('data-login_uri');
            const initKey = `${clientId}|${loginUri}`;
            if (googleIdentityInitKey !== initKey) {
              google.accounts.id.initialize({
                client_id: clientId,
                callback: undefined,
                login_uri: loginUri,
                ux_mode: 'redirect',
              });
              googleIdentityInitKey = initKey;
            }
            google.accounts.id.renderButton(gIdSignin, {
              theme: 'outline',
              size: 'large',
              width: 280,
            });
          }
        } catch (e) {
          console.warn('Google Identity initialization skipped', e);
          if (gIdOnload) gIdOnload.remove();
          if (gIdSignin) gIdSignin.remove();
        }
      } else {
        if (gIdOnload) gIdOnload.remove();
        if (gIdSignin) gIdSignin.remove();
      }
    }
  } catch (e) {
    console.warn('Could not fetch config', e);
  }

  document.getElementById('login-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const username = e.target.username.value;
    const password = e.target.password.value;

    try {
      const { res, data } = await apiJson('/auth/dev/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      });

      if (res.ok) {
        const profile = data && data.user ? data.user : data;

        const previousUserId = getCurrentUserId();
        if (profile) setCurrentUser(profile);
        const nextUserId = profile && profile.id ? profile.id : null;
        if (!previousUserId || (nextUserId && nextUserId !== previousUserId)) {
          await clearUserCaches();
        }
        setAuthenticated(true);
        restoreAppShellIfNeeded();
        document.getElementById('nav-container').classList.remove('hidden', 'sm:hidden');
        document.getElementById('auth-actions').classList.remove('hidden');

        try {
          await Sync.pushChanges();
        } catch (err) {
          console.warn('Initial push changes failed', err);
        }
        try {
          await refreshCharitiesCache();
        } catch (err) {
          console.warn('Failed to refresh charities on login', err);
        }
        try {
          await refreshDonationsFromServer();
        } catch (err) {
          console.warn('Failed to refresh donations on login', err);
        }

        await updateTotals();

        const goto = consumeReturnTo() || '/';
        await navigate(goto);
      } else {
        alert('Login failed');
      }
    } catch (err) {
      console.error(err);
      alert('Error during login');
    }
  });
}
