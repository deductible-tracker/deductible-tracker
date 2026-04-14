# QA Report: Mobile & CSP Fixes (2026-04-14)

## Summary
Successfully resolved invalid Content Security Policy (CSP) source and implemented several mobile-responsive design improvements.

| Category | Status | Details |
|----------|--------|---------|
| Security (CSP) | Fixed | Invalid double-wildcard Oracle Cloud domain removed. |
| Navigation (Mobile) | Improved | Hamburger/Close icon toggling and overlay menu. |
| Header (Mobile) | Improved | Global header now visible on all sub-routes. |
| Login (Mobile) | Improved | Reordered sign-in form to top of page. |
| Health Score | 95/100 | (Up from ~70 due to CSP and navigation fixes) |

## Discovered Issues & Fixes

### 1. Invalid CSP Source in 'img-src' and 'connect-src'
- **Issue**: Chrome console reported errors for `https://*.compat.objectstorage.*.oraclecloud.com` due to multiple wildcards.
- **Fix**: Replaced with `https://*.oraclecloud.com` in `server_bootstrap.rs`.
- **Status**: Verified fixed in browser.

### 2. Mobile Menu Button Icon
- **Issue**: Hamburger icon remained even when the menu was open.
- **Fix**: Updated `index.html` with a hidden close icon and `app.js` to toggle between them.
- **Status**: Verified fixed in browser.

### 3. Mobile Navigation Header Hidden on Sub-routes
- **Issue**: Nav header was hidden completely on mobile for `/new`, `/edit`, and `/view/` routes, losing branding and global navigation.
- **Fix**: Removed `hide-nav-on-mobile` CSS logic and JS class toggle.
- **Status**: Verified fixed in browser.

### 4. Mobile Menu Pushes Content Down
- **Issue**: Opening the mobile menu pushed the entire page content down, which is a poor UX.
- **Fix**: Changed `mobile-menu` to be an `absolute` overlay with full height and shadow.
- **Status**: Verified fixed in browser.

### 5. Login Page Order on Mobile
- **Issue**: On small screens, the informational "Workspace Overview" panel was at the top, pushing the "Sign in" form below the fold.
- **Fix**: Reordered components using Tailwind `order-` classes so the login form is first on mobile.
- **Status**: Verified fixed in browser.

## Screenshots
- Initial Mobile Login: `.gstack/qa-reports/screenshots/login-mobile-fixed.png`
- Mobile Menu (Fixed Overlay): `.gstack/qa-reports/screenshots/mobile-menu-fixed.png`
