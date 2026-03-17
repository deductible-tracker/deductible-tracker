import db from './db.js';
import { Sync } from './sync.js';
import { apiJson, getCookie } from './services/http.js';
import {
  captureAppShellTemplate as captureAppShellTemplateFromElement,
  restoreAppShellIfMissingRouteContent,
} from './services/app-shell.js';
import {
  formatCurrency,
  formatFigureText,
  calculateDonationFigures,
} from './services/donation-figures.js';
import {
  createDonationOnServer,
  createOrGetCharityOnServer,
  deleteCharityOnServer,
  deleteDonationOnServer,
  fetchCharitiesFromServer,
  lookupCharityByEinOnServer,
  updateCharityOnServer,
  updateDonationOnServer,
} from './services/api-client.js';
import { calculateTaxEstimates } from './services/tax-estimates.js';
import {
  clearCurrentUser,
  getCurrentUser,
  getCurrentUserId,
  setCurrentUser,
} from './services/current-user.js';
import { iconSvg } from './services/icons.js';
import { escapeHtml } from './utils/html.js';
import { renderDashboardRoute, renderRecentListRoute } from './views/routes/dashboard.js';
import {
  renderDonationEditRoute,
  renderDonationNewRoute,
  renderDonationsRoute,
  renderReceiptPageRoute,
} from './views/routes/donations.js';
import {
  renderCharitiesRoute,
  renderCharityEditRoute,
  renderCharityNewRoute,
} from './views/routes/charities.js';
import { renderReportsRoute } from './views/routes/reports.js';
import { renderPersonalInfoRoute } from './views/routes/personal.js';

// Simple Router
const routes = {
  '/': renderDashboard,
  '/donations': renderDonations,
  '/charities': renderCharities,
  '/reports': renderReports,
  '/personal': renderPersonalInfo,
};

// Authentication state (cached to avoid extra requests on every click)
let AUTHENTICATED = false;
// Route to return to after successful login
let RETURN_TO = null;
let APP_SHELL_TEMPLATE = '';
const CHARITY_CACHE_TTL_MS = 1000 * 60 * 60 * 24 * 30; // 30 days
// Cached auth check to avoid spamming /api/me (rate-limited by governor)
let lastAuthCheckAt = 0;
let lastAuthResult = false;
let authCheckInFlight = null;
let lastUserFetchAt = 0;

function captureAppShellTemplate() {
  if (APP_SHELL_TEMPLATE) return;
  const app = document.getElementById('app');
  APP_SHELL_TEMPLATE = captureAppShellTemplateFromElement(app);
}

function restoreAppShellIfNeeded() {
  const app = document.getElementById('app');
  restoreAppShellIfMissingRouteContent(app, APP_SHELL_TEMPLATE);
}

function getProfileStorageKey() {
  const userId = getCurrentUserId();
  return userId ? `profile:${userId}` : 'profile:anonymous';
}

async function clearUserCaches() {
  try {
    await db.donations.clear();
  } catch (e) {
    /* ignore */
  }
  try {
    await db.receipts.clear();
  } catch (e) {
    /* ignore */
  }
  try {
    await db.charities.clear();
  } catch (e) {
    /* ignore */
  }
  // NOTE: do not clear `sync_queue` here so pending offline changes are not lost on logout
}

async function checkAuthCached() {
  const now = Date.now();
  const ttlMs = 4000;
  if (authCheckInFlight) return authCheckInFlight;
  if (now - lastAuthCheckAt < ttlMs) return lastAuthResult;
  authCheckInFlight = (async () => {
    try {
      const res = await fetch('/api/me', { credentials: 'include' });
      // Treat 429 as "unknown" and keep the last known result
      if (res.status === 429) return lastAuthResult;

      // SILENCE logic: only console.error if it's NOT a 401 (which is expected if not logged in)
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

async function refreshDonationsFromServer() {
  const userId = getCurrentUserId();
  if (!userId) return;
  const { res, data } = await apiJson('/api/donations');
  if (!res.ok || !data || !data.donations) return;
  const donations = data.donations.map((d) => ({
    id: d.id,
    user_id: userId,
    year: d.year,
    date: d.date,
    category: d.category || 'money',
    amount: d.amount ?? 0,
    charity_id: d.charity_id,
    notes: d.notes || null,
    sync_status: 'synced',
    updated_at: d.updated_at || null,
    created_at: d.created_at || null,
  }));
  try {
    await db.donations.where('user_id').equals(userId).delete();
    await db.donations.bulkPut(donations);
  } catch (e) {
    /* ignore */
  }
  await refreshReceiptsFromServer(donations);
}

async function refreshReceiptsFromServer(donations = []) {
  const userId = getCurrentUserId();
  if (!userId) return;
  try {
    const { res, data } = await apiJson('/api/receipts');
    if (!res.ok || !data || !data.receipts) return;
    const receipts = data.receipts.map((r) => ({
      id: r.id,
      key: r.key,
      file_name: r.file_name || null,
      content_type: r.content_type || null,
      size: r.size || null,
      donation_id: r.donation_id,
      uploaded_at: r.created_at || new Date().toISOString(),
    }));
    try {
      await db.transaction('rw', db.receipts, async () => {
        if (donations.length > 0) {
          await db.receipts
            .where('donation_id')
            .anyOf(donations.map((d) => d.id))
            .delete();
        } else {
          await db.receipts.clear();
        }
        await db.receipts.bulkPut(receipts);
      });
    } catch (e) {
      /* ignore */
    }
  } catch (e) {
    console.error('Failed to refresh receipts', e);
  }
}

async function refreshCharitiesCache() {
  const userId = getCurrentUserId();
  if (!userId) return [];
  const list = await fetchCharitiesFromServer();
  const now = Date.now();
  const cached = list.map((c) => ({
    id: c.id,
    user_id: userId,
    name: c.name,
    ein: c.ein || '',
    category: c.category || null,
    status: c.status || null,
    classification: c.classification || null,
    nonprofit_type: c.nonprofit_type || null,
    deductibility: c.deductibility || null,
    street: c.street || null,
    city: c.city || null,
    state: c.state || null,
    zip: c.zip || null,
    cached_at: now,
  }));
  try {
    await db.charities.where('user_id').equals(userId).delete();
    await db.charities.bulkPut(cached);
  } catch (e) {
    /* ignore */
  }
  return cached;
}

function normalizeRoute(path) {
  const raw = (path || '/').toString();
  const pathname = raw.split('?')[0] || '/';
  if (pathname === '/index.html') return '/';
  if (routes[pathname]) return pathname;
  if (/^\/donations\/new$/.test(pathname)) return pathname;
  if (/^\/donations\/edit\/[^/]+$/.test(pathname)) return pathname;
  if (/^\/donations\/receipts\/[^/]+$/.test(pathname)) return pathname;
  if (/^\/charities\/new$/.test(pathname)) return pathname;
  if (/^\/charities\/edit\/[^/]+$/.test(pathname)) return pathname;
  return '/';
}

function updateHomeSummaryVisibility(path) {
  const summary = document.getElementById('home-summary');
  if (!summary) return;
  const routeContent = document.getElementById('route-content');
  const isHome = normalizeRoute(path) === '/';
  summary.classList.toggle('hidden', !isHome);
  if (routeContent) {
    routeContent.classList.toggle('mt-8', isHome);
    routeContent.classList.toggle('mt-0', !isHome);
  }
}

async function navigate(path, options = {}) {
  const { pushState = true } = options;
  const target = normalizeRoute(path);
  // Prevent navigating into protected routes when not authenticated
  if (!AUTHENTICATED) {
    // remember where the user wanted to go and show login
    RETURN_TO = target;
    renderLogin();
    return;
  }
  if (pushState) {
    window.history.pushState({}, '', target);
  }
  updateHomeSummaryVisibility(target);

  // Toggle nav visibility on mobile based on route
  if (target.includes('/new') || target.includes('/edit') || target.includes('/receipts/')) {
    document.body.classList.add('hide-nav-on-mobile');
  } else {
    document.body.classList.remove('hide-nav-on-mobile');
  }

  let handler;
  if (routes[target]) {
    handler = routes[target];
  } else {
    let m;
    if ((m = target.match(/^\/donations\/edit\/([^/]+)$/))) {
      handler = () => renderDonationEdit(decodeURIComponent(m[1]));
    } else if (/^\/donations\/new$/.test(target)) {
      handler = renderDonationNew;
    } else if ((m = target.match(/^\/donations\/receipts\/([^/]+)$/))) {
      handler = () => renderReceiptPage(decodeURIComponent(m[1]));
    } else if ((m = target.match(/^\/charities\/edit\/([^/]+)$/))) {
      handler = () => renderCharityEdit(decodeURIComponent(m[1]));
    } else if (/^\/charities\/new$/.test(target)) {
      handler = renderCharityNew;
    } else {
      handler = routes['/'];
    }
  }
  await handler();
  updateActiveLink(target);
}

function updateActiveLink(path) {
  const navRoute = path.startsWith('/donations/')
    ? '/donations'
    : path.startsWith('/charities/')
      ? '/charities'
      : path;
  document.querySelectorAll('a[data-route]').forEach((a) => {
    if (a.dataset.route === navRoute) {
      a.classList.add('bg-indigo-600', 'text-white', 'dark:bg-indigo-500');
      a.classList.remove(
        'text-slate-700',
        'dark:text-slate-300',
        'hover:bg-indigo-50',
        'hover:text-indigo-700',
        'dark:hover:bg-slate-800',
        'dark:hover:text-indigo-400'
      );
    } else {
      a.classList.remove('bg-indigo-600', 'text-white', 'dark:bg-indigo-500');
      a.classList.add(
        'text-slate-700',
        'dark:text-slate-300',
        'hover:bg-indigo-50',
        'hover:text-indigo-700',
        'dark:hover:bg-slate-800',
        'dark:hover:text-indigo-400'
      );
    }
  });
}

// --- Views ---

async function renderLogin() {
  const app = document.getElementById('app');
  app.innerHTML = `
        <div class="mx-auto grid min-h-full max-w-6xl items-center gap-8 py-8 sm:py-12 lg:grid-cols-2">
            <div class="rounded-2xl border border-slate-200 dark:border-slate-700 bg-linear-to-br from-indigo-50 to-white p-6 sm:p-10">
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
            <div class="dt-panel p-6 sm:p-8">
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
      // If Google Sign-In is enabled and a client ID is provided, dynamically
      // load the Google Identity script and initialize the button. This avoids
      // the Google script running on origins that are not configured for the
      // client ID (which causes 403/origin errors in the console).
      if (config.google_enabled && config.google_client_id) {
        try {
          // Set attributes for the element
          gIdOnload.setAttribute('data-client_id', config.google_client_id);
          // Use location.origin to ensure the login_uri is absolute, as required by Google OAuth policies.
          if (!gIdOnload.getAttribute('data-login_uri')) {
            gIdOnload.setAttribute('data-login_uri', `${location.origin}/auth/callback/google`);
          }

          // Dynamically load the Google Identity script only when configured
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
            google.accounts.id.initialize({
              client_id: gIdOnload.getAttribute('data-client_id'),
              callback: undefined, // Redirect mode
              login_uri: gIdOnload.getAttribute('data-login_uri'),
              ux_mode: 'redirect',
            });
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
        // If not configured, remove the Google sign-in elements to avoid
        // any attempted loads or visual empty slots.
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
        let profile = null;
        profile = data && data.user ? data.user : data;

        const previousUserId = getCurrentUserId();
        if (profile) setCurrentUser(profile);
        const nextUserId = profile && profile.id ? profile.id : null;
        if (!previousUserId || (nextUserId && nextUserId !== previousUserId)) {
          await clearUserCaches();
        }
        AUTHENTICATED = true;
        restoreAppShellIfNeeded();
        document.getElementById('nav-container').classList.remove('hidden', 'sm:hidden');
        document.getElementById('auth-actions').classList.remove('hidden');

        // After login, ensure local IndexedDB is aligned with server state
        try {
          await Sync.pushChanges(); // push any pending local changes first
        } catch (e) {
          console.warn('Initial push changes failed', e);
        }
        try {
          await refreshCharitiesCache();
        } catch (e) {
          console.warn('Failed to refresh charities on login', e);
        }
        try {
          await refreshDonationsFromServer();
        } catch (e) {
          console.warn('Failed to refresh donations on login', e);
        }

        await updateTotals();

        const goto = RETURN_TO || '/';
        RETURN_TO = null;
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

async function getUserDonations() {
  const userId = getCurrentUserId();
  if (!userId) return [];
  return db.donations.where('user_id').equals(userId).toArray();
}

async function getUserCharityNameMap() {
  const userId = getCurrentUserId();
  const map = new Map();
  if (!userId) return map;
  const charities = await db.charities.where('user_id').equals(userId).toArray();
  for (const charity of charities) {
    if (charity && charity.id) {
      map.set(charity.id, charity.name || 'Unknown charity');
    }
  }
  return map;
}

function isCharityCacheFresh(entry) {
  if (!entry || !entry.cached_at) return false;
  return Date.now() - entry.cached_at <= CHARITY_CACHE_TTL_MS;
}

function buildRouteDeps() {
  return {
    apiJson,
    getCookie,
    calculateDonationFigures,
    calculateTaxEstimates,
    createDonationOnServer,
    createOrGetCharityOnServer,
    db,
    deleteCharityOnServer,
    deleteDonationOnServer,
    escapeHtml,
    formatCurrency,
    formatFigureText,
    getCurrentUser,
    getCurrentUserId,
    getUserCharityNameMap,
    getUserDonations,
    isCharityCacheFresh,
    lookupCharityByEinOnServer,
    navigate,
    refreshCharitiesCache,
    setCurrentUser,
    Sync,
    updateCharityOnServer,
    updateDonationOnServer,
    updateTotals,
  };
}

async function handleLogout() {
  try {
    // Invalidate server cookie/session
    await apiJson('/auth/logout', { method: 'POST' });
  } catch (e) {
    console.warn('Logout request failed', e);
  }

  // Clear any client-side state if needed (e.g., hide UI)
  const nav = document.getElementById('nav-container');
  if (nav) nav.classList.add('hidden');
  const authActions = document.getElementById('auth-actions');
  if (authActions) authActions.classList.add('hidden');

  AUTHENTICATED = false;

  // Clear all user-scoped caches and profile data
  const profileKey = getProfileStorageKey();
  await clearUserCaches();
  try {
    localStorage.removeItem(profileKey);
  } catch (e) {
    /* ignore */
  }
  clearCurrentUser();
  lastAuthResult = false;
  lastAuthCheckAt = 0;
  lastUserFetchAt = 0;

  // Render login view
  renderLogin();

  // Force the URL to the public root so protected routes are not visible
  try {
    window.location.replace('/');
  } catch (e) {
    /* ignore */
  }
}

async function renderDashboard() {
  await renderDashboardRoute({ ...buildRouteDeps(), renderRecentList });
}

async function renderRecentList() {
  await renderRecentListRoute(buildRouteDeps());
}

async function renderDonations() {
  await renderDonationsRoute(buildRouteDeps());
}

async function renderDonationNew() {
  await renderDonationNewRoute(buildRouteDeps());
}

async function renderDonationEdit(donationId) {
  await renderDonationEditRoute(donationId, buildRouteDeps());
}

async function renderReceiptPage(donationId) {
  await renderReceiptPageRoute(donationId, buildRouteDeps());
}

async function renderReports() {
  await renderReportsRoute(buildRouteDeps());
}

async function renderCharities() {
  await renderCharitiesRoute(buildRouteDeps());
}
async function renderCharityNew() {
  await renderCharityNewRoute(buildRouteDeps());
}

async function renderCharityEdit(charityId) {
  await renderCharityEditRoute(charityId, buildRouteDeps());
}

async function renderPersonalInfo() {
  await renderPersonalInfoRoute(buildRouteDeps());
}

// Update top-level totals in the new dashboard shell
async function updateTotals() {
  try {
    const donations = await getUserDonations();
    const figures = calculateDonationFigures(donations);
    const userId = getCurrentUserId();
    const charities = userId ? await db.charities.where('user_id').equals(userId).toArray() : [];
    const receipts = await db.receipts.toArray();
    const taxEstimates = await calculateTaxEstimates(
      donations,
      charities,
      receipts,
      getCurrentUser() || {}
    );
    const totalEl = document.getElementById('total-donations-amount');
    const estEl = document.getElementById('estimated-savings');
    const itemsEl = document.getElementById('figure-items-amount');
    const moneyEl = document.getElementById('figure-money-amount');
    const mileageEl = document.getElementById('figure-mileage-amount');

    if (totalEl) totalEl.textContent = formatCurrency(figures.total.amount);
    if (estEl) {
      estEl.textContent = `${formatCurrency(taxEstimates.totalEstimated)} in estimated tax savings`;
    }
    if (itemsEl) itemsEl.textContent = formatCurrency(figures.items.amount);
    if (moneyEl) moneyEl.textContent = formatCurrency(figures.money.amount);
    if (mileageEl) mileageEl.textContent = formatCurrency(figures.mileage.amount);
  } catch (e) {
    console.error('Failed to update totals', e);
  }
}

async function updateSyncStatus() {
  const statusEl = document.getElementById('sync-status');
  if (!statusEl) return;
  const isOnline = navigator.onLine;
  const userId = getCurrentUserId();
  let pending = 0;
  try {
    if (userId) pending = await Sync.countPendingChanges(userId);
  } catch (e) {
    /* ignore */
  }

  const pendingLabel = pending > 0 ? ` • ${pending} pending` : '';
  if (isOnline) {
    statusEl.innerHTML = `${iconSvg('cloud', 'mr-1 h-4 w-4 text-green-500')} Online${pendingLabel}${pending > 0 ? ' <button id="sync-now-btn" class="ml-2 text-xs text-blue-600 underline">Sync now</button>' : ''}`;
  } else {
    statusEl.innerHTML = `${iconSvg('cloud-off', 'mr-1 h-4 w-4 text-red-500')} Offline${pendingLabel}`;
  }

  if (pending > 0 && isOnline) {
    const btn = document.getElementById('sync-now-btn');
    if (btn) btn.addEventListener('click', () => Sync.pushChanges());
  }
}

// --- Init ---
async function init() {
  console.log('App initializing...');
  captureAppShellTemplate();

  try {
    await db.open();
  } catch (e) {
    const schemaResetKey = 'dexie_schema_reset_done';
    const errorName = e && e.name ? e.name : '';
    const message = e && e.message ? e.message : String(e);
    const isSchemaMismatch =
      message.includes('not indexed') ||
      message.includes('KeyPath') ||
      message.includes('primary key') ||
      errorName === 'UpgradeError';

    if (isSchemaMismatch) {
      const alreadyReset = sessionStorage.getItem(schemaResetKey) === '1';
      if (alreadyReset) {
        console.error(
          'Dexie schema reset already attempted for this session; aborting retry loop.',
          e
        );
        throw e;
      }

      console.warn('Dexie schema mismatch detected. Clearing local database and reloading.');
      sessionStorage.setItem(schemaResetKey, '1');
      try {
        await db.delete();
      } catch (de) {
        /* ignore */
      }
      window.location.reload();
      return;
    }

    sessionStorage.removeItem(schemaResetKey);
    throw e;
  }

  try {
    sessionStorage.removeItem('dexie_schema_reset_done');
  } catch (_) {
    /* ignore */
  }

  // 1. Network Status & Initial UI State
  const updateStatus = async () => {
    const isOnline = navigator.onLine;
    if (isOnline) {
      Sync.pushChanges().catch((err) => console.error('Initial sync failed:', err));
    }
    await updateSyncStatus();
  };

  window.addEventListener('online', updateStatus);
  window.addEventListener('offline', updateStatus);
  window.addEventListener('sync-queue-changed', updateSyncStatus);
  updateStatus();

  // 2. Global Event Listeners (Attach immediately)
  // Nav link routing (works for top nav and mobile menu)
  document.querySelectorAll('[data-route]').forEach((a) => {
    a.addEventListener('click', async (e) => {
      e.preventDefault();
      const link = e.currentTarget;
      const route = link ? link.dataset.route : null;
      // If mobile menu visible, hide it on navigation
      const mobile = document.getElementById('mobile-menu');
      if (mobile && !mobile.classList.contains('hidden')) mobile.classList.add('hidden');

      // Verify with server whether the user is still authenticated
      try {
        const isAuthed = await checkAuthCached();
        if (!isAuthed) {
          AUTHENTICATED = false;
          RETURN_TO = route;
          renderLogin();
          return;
        }
        // mark authenticated and navigate
        AUTHENTICATED = true;
        if (route) await navigate(route);
      } catch (err) {
        console.warn('Auth check failed', err);
        // On error, keep current state and show login as fallback
        AUTHENTICATED = false;
        RETURN_TO = route;
        renderLogin();
      }
    });
  });

  // Mobile menu toggle
  const mobileButton = document.getElementById('mobile-menu-button');
  if (mobileButton) {
    mobileButton.addEventListener('click', () => {
      const mobile = document.getElementById('mobile-menu');
      if (!mobile) return;
      mobile.classList.toggle('hidden');
    });
  }

  const btnAddDonation = document.getElementById('btn-add-donation');
  if (btnAddDonation) {
    btnAddDonation.addEventListener('click', () => navigate('/donations/new'));
  }
  const btnAddDonationMobile = document.getElementById('btn-add-donation-mobile');
  if (btnAddDonationMobile) {
    btnAddDonationMobile.addEventListener('click', async () => {
      const mobile = document.getElementById('mobile-menu');
      if (mobile) mobile.classList.add('hidden');
      await navigate('/donations/new');
    });
  }

  const btnLogout = document.getElementById('btn-logout');
  if (btnLogout) {
    btnLogout.addEventListener('click', handleLogout);
  }
  const btnLogoutMobile = document.getElementById('btn-logout-mobile');
  if (btnLogoutMobile) {
    btnLogoutMobile.addEventListener('click', async () => {
      const mobile = document.getElementById('mobile-menu');
      if (mobile) mobile.classList.add('hidden');
      await handleLogout();
    });
  }

  // Ensure back/forward navigation also respects auth
  window.addEventListener('popstate', async (_e) => {
    const path = location.pathname;
    try {
      const isAuthed = await checkAuthCached();
      if (!isAuthed) {
        AUTHENTICATED = false;
        RETURN_TO = path;
        window.history.replaceState({}, '', '/');
        renderLogin();
        return;
      }
      AUTHENTICATED = true;
      await navigate(path, { pushState: false });
    } catch (err) {
      console.warn('Auth check failed on popstate', err);
      AUTHENTICATED = false;
      RETURN_TO = path;
      window.history.replaceState({}, '', '/');
      renderLogin();
    }
  });

  try {
    // update shell totals
    await updateTotals();

    // 4. Auth Check
    const isAuthed = await checkAuthCached();
    if (!isAuthed) {
      console.log('Not authenticated, rendering login');
      AUTHENTICATED = false;
      document.getElementById('nav-container').classList.add('hidden');
      document.getElementById('auth-actions').classList.add('hidden');
      updateHomeSummaryVisibility('/login');
      await clearUserCaches();
      clearCurrentUser();
      const requestedPath = location.pathname;
      if (requestedPath !== '/' && requestedPath !== '/index.html') {
        RETURN_TO = requestedPath;
        window.history.replaceState({}, '', '/');
      }
      renderLogin();
    } else {
      console.log('Authenticated, navigating to route');
      AUTHENTICATED = true;
      if (!getCurrentUserId()) {
        try {
          const res = await fetch('/api/me', { credentials: 'include' });
          if (res.ok) {
            const profile = await res.json();
            setCurrentUser(profile);
          }
        } catch (e) {
          /* ignore */
        }
      }
      try {
        // Ensure server has valuation suggestions seeded (best-effort)
        try {
          await apiJson('/api/valuations/seed', { method: 'POST' });
        } catch (e) {
          /* ignore */
        }
        await refreshCharitiesCache();
        await refreshDonationsFromServer();
      } catch (e) {
        /* ignore */
      }
      document.getElementById('nav-container').classList.remove('hidden', 'sm:hidden');
      document.getElementById('auth-actions').classList.remove('hidden');
      const initialRoute =
        location.pathname === '/index.html' || location.pathname === '/' ? '/' : location.pathname;
      await navigate(initialRoute);
    }
  } catch (err) {
    console.error('Initialization failed:', err);
    // Fallback: show something so the user isn't stuck at "Loading..."
    const safeMessage = escapeHtml(err && err.message ? err.message : 'Unknown error');
    document.getElementById('app').innerHTML = `
            <div class="p-8 text-center">
                <h1 class="text-rose-600 dark:text-rose-400 font-semibold">Initialization Error</h1>
                <p class="text-slate-600 dark:text-slate-300">${safeMessage}</p>
                <button onclick="location.reload()" class="dt-btn-primary mt-4">Retry</button>
            </div>
        `;
  }
}

// Run init when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
