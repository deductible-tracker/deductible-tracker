/**
 * Client-side router.
 *
 * Owns route normalization, the route table, navigation with auth
 * gating, active-link styling, and history management.
 */

import {
  isAuthenticated,
  setAuthenticated,
  setReturnTo,
  checkAuthCached,
} from '../services/auth-state.js';

let routeTable = {};
let paramRouteHandler = null;

export function setRoutes(routes) {
  routeTable = routes;
}

export function setParamRouteHandler(handler) {
  paramRouteHandler = handler;
}

export function normalizeRoute(path) {
  const raw = (path || '/').toString();
  const pathname = raw.split('?')[0] || '/';
  if (pathname === '/index.html') return '/';
  if (routeTable[pathname]) return pathname;
  if (/^\/donations\/new$/.test(pathname)) return pathname;
  if (/^\/donations\/edit\/[^/]+$/.test(pathname)) return pathname;
  if (/^\/donations\/view\/[^/]+$/.test(pathname)) return pathname;
  if (/^\/charities\/new$/.test(pathname)) return pathname;
  if (/^\/charities\/edit\/[^/]+$/.test(pathname)) return pathname;
  if (/^\/charities\/view\/[^/]+$/.test(pathname)) return pathname;
  return '/';
}

export function updateHomeSummaryVisibility(path) {
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

export async function navigate(path, options = {}) {
  const { pushState = true } = options;
  const target = normalizeRoute(path);

  if (!isAuthenticated()) {
    setReturnTo(target);
    // Caller must handle rendering login — import renderLogin in init
    window.dispatchEvent(new CustomEvent('dt-show-login'));
    return;
  }
  if (pushState) {
    window.history.pushState({}, '', target);
  }
  updateHomeSummaryVisibility(target);

  let handler;
  if (routeTable[target]) {
    handler = routeTable[target];
  } else if (paramRouteHandler) {
    handler = paramRouteHandler(target);
  }

  if (!handler) {
    handler = routeTable['/'];
  }

  await handler();
  updateActiveLink(target);
}

export function updateActiveLink(path) {
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

/**
 * Bind popstate and nav-link click handlers.
 */
export function initRouterListeners({ renderLogin, closeMobileMenu }) {
  document.querySelectorAll('[data-route]').forEach((a) => {
    a.addEventListener('click', async (e) => {
      e.preventDefault();
      const link = e.currentTarget;
      const route = link ? link.dataset.route : null;
      closeMobileMenu();

      try {
        const isAuthed = await checkAuthCached();
        if (!isAuthed) {
          setAuthenticated(false);
          setReturnTo(route);
          renderLogin();
          return;
        }
        setAuthenticated(true);
        if (route) await navigate(route);
      } catch (err) {
        console.warn('Auth check failed', err);
        setAuthenticated(false);
        setReturnTo(route);
        renderLogin();
      }
    });
  });

  window.addEventListener('popstate', async () => {
    const path = location.pathname;
    try {
      const isAuthed = await checkAuthCached();
      if (!isAuthed) {
        setAuthenticated(false);
        setReturnTo(path);
        window.history.replaceState({}, '', '/');
        renderLogin();
        return;
      }
      setAuthenticated(true);
      await navigate(path, { pushState: false });
    } catch (err) {
      console.warn('Auth check failed on popstate', err);
      setAuthenticated(false);
      setReturnTo(path);
      window.history.replaceState({}, '', '/');
      renderLogin();
    }
  });
}
