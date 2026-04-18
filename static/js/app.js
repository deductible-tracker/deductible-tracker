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
import { registerVaultKey, unlockVaultKey, encryptData, decryptData, isWebAuthnSupported } from './services/crypto.js';

// Extracted modules
import {
  setAuthenticated,
  setReturnTo,
  checkAuthCached,
  handleLogout as authHandleLogout,
} from './services/auth-state.js';
import {
  clearUserCaches,
  refreshDonationsFromServer,
  refreshCharitiesCache,
  isCharityCacheFresh,
} from './services/cache-manager.js';
import {
  setRoutes,
  setParamRouteHandler,
  navigate,
  updateHomeSummaryVisibility,
  initRouterListeners,
} from './router.js';
import { renderLoginView } from './views/login.js';

// View route renderers
import { renderDashboardRoute, renderRecentListRoute } from './views/routes/dashboard.js';
import {
  renderDonationEditRoute,
  renderDonationNewRoute,
  renderDonationsRoute,
  renderDonationViewRoute,
} from './views/routes/donations.js';
import {
  renderCharitiesRoute,
  renderCharityEditRoute,
  renderCharityNewRoute,
  renderCharityViewRoute,
} from './views/routes/charities.js';
import { renderReportsRoute } from './views/routes/reports.js';
import { renderPersonalInfoRoute } from './views/routes/personal.js';

// --- App shell ---

let APP_SHELL_TEMPLATE = '';

function captureAppShellTemplate() {
  if (APP_SHELL_TEMPLATE) return;
  const app = document.getElementById('app');
  APP_SHELL_TEMPLATE = captureAppShellTemplateFromElement(app);
}

function restoreAppShellIfNeeded() {
  const app = document.getElementById('app');
  restoreAppShellIfMissingRouteContent(app, APP_SHELL_TEMPLATE);
}

// --- Data helpers ---

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

// --- View wrappers ---

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
    registerVaultKey,
    unlockVaultKey,
    encryptData,
    decryptData,
    isWebAuthnSupported,
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
    handleLogout,
    Sync,
    updateCharityOnServer,
    updateDonationOnServer,
    updateTotals,
  };
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
async function renderDonationView(donationId) {
  await renderDonationViewRoute(donationId, buildRouteDeps());
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
async function renderCharityView(charityId) {
  await renderCharityViewRoute(charityId, buildRouteDeps());
}
async function renderPersonalInfo() {
  await renderPersonalInfoRoute(buildRouteDeps());
}

function renderLogin() {
  renderLoginView({
    clearUserCaches,
    navigate,
    refreshCharitiesCache,
    refreshDonationsFromServer,
    restoreAppShellIfNeeded,
    Sync,
    updateTotals,
  });
}

async function handleLogout() {
  await authHandleLogout({ clearUserCaches, renderLogin });
}

// --- Dashboard totals ---

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

  // Register routes with the router
  setRoutes({
    '/': renderDashboard,
    '/donations': renderDonations,
    '/charities': renderCharities,
    '/reports': renderReports,
    '/personal': renderPersonalInfo,
  });

  setParamRouteHandler((target) => {
    let m;
    if ((m = target.match(/^\/donations\/edit\/([^/]+)$/))) {
      return () => renderDonationEdit(decodeURIComponent(m[1]));
    } else if ((m = target.match(/^\/donations\/view\/([^/]+)$/))) {
      return () => renderDonationView(decodeURIComponent(m[1]));
    } else if (/^\/donations\/new$/.test(target)) {
      return renderDonationNew;
    } else if ((m = target.match(/^\/charities\/edit\/([^/]+)$/))) {
      return () => renderCharityEdit(decodeURIComponent(m[1]));
    } else if (/^\/charities\/new$/.test(target)) {
      return renderCharityNew;
    } else if ((m = target.match(/^\/charities\/view\/([^/]+)$/))) {
      return () => renderCharityView(decodeURIComponent(m[1]));
    }
    return null;
  });

  // Listen for the router's login request
  window.addEventListener('dt-show-login', () => renderLogin());

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

  function closeMobileMenu() {
    const mobile = document.getElementById('mobile-menu');
    if (mobile && !mobile.classList.contains('hidden')) {
      mobile.classList.add('hidden');
      document.body.classList.remove('nav-open');
      document.getElementById('hamburger-icon')?.classList.remove('hidden');
      document.getElementById('close-icon')?.classList.add('hidden');
    }
  }

  // 2. Global Event Listeners
  initRouterListeners({ renderLogin, closeMobileMenu });

  // Mobile menu toggle
  const mobileButton = document.getElementById('mobile-menu-button');
  const hamburgerIcon = document.getElementById('hamburger-icon');
  const closeIcon = document.getElementById('close-icon');

  if (mobileButton) {
    mobileButton.addEventListener('click', () => {
      const mobile = document.getElementById('mobile-menu');
      if (!mobile) return;
      const isOpen = !mobile.classList.contains('hidden');
      if (isOpen) {
        closeMobileMenu();
      } else {
        mobile.classList.remove('hidden');
        document.body.classList.add('nav-open');
        hamburgerIcon?.classList.add('hidden');
        closeIcon?.classList.remove('hidden');
      }
    });
  }

  const btnAddDonation = document.getElementById('btn-add-donation');
  if (btnAddDonation) {
    btnAddDonation.addEventListener('click', () => navigate('/donations/new'));
  }
  const btnAddDonationMobile = document.getElementById('btn-add-donation-mobile');
  if (btnAddDonationMobile) {
    btnAddDonationMobile.addEventListener('click', async () => {
      closeMobileMenu();
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
      closeMobileMenu();
      await handleLogout();
    });
  }

  try {
    await updateTotals();

    // Auth Check
    const isAuthed = await checkAuthCached();
    if (!isAuthed) {
      console.log('Not authenticated, rendering login');
      setAuthenticated(false);
      document.getElementById('nav-container').classList.add('hidden');
      document.getElementById('auth-actions').classList.add('hidden');
      updateHomeSummaryVisibility('/login');
      await clearUserCaches();
      clearCurrentUser();
      const requestedPath = location.pathname;
      if (requestedPath !== '/' && requestedPath !== '/index.html') {
        setReturnTo(requestedPath);
        window.history.replaceState({}, '', '/');
      }
      renderLogin();
    } else {
      console.log('Authenticated, navigating to route');
      setAuthenticated(true);
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
