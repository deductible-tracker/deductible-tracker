---
name: frontend-feature
description: Use when building or changing deductible-tracker frontend features, including Dexie.js schema updates, services, sync, and view rendering.
---

# Skill: Adding a new Frontend Feature

This skill provides a structured approach for developing new client-side features in the `deductible-tracker` web application.

## 1. Local Database (Dexie.js)

If the feature requires persistence, start by defining the schema in `static/js/db.js`.

- Add or update the table definition in `db.version(X).stores(...)`.
- Bump the version if the schema changes.
- Remember: `id` is the primary key for IndexedDB.

## 2. Implement the Service

Logic and API calls should be isolated in `static/js/services/`.

- Use `static/js/services/api-client.js` for server interaction.
- Use other services (e.g., `donation-figures.js`) for business calculations.
- Follow the pattern in `static/js/sync.js` for optimistic UI:
  - Apply the change to Dexie immediately.
  - Queue the change in `db.sync_queue` for background sync.

## 3. View & Navigation

The UI is a Single Page Application (SPA) with a simple router.

- Define the HTML structure in a view function (e.g., `renderMyFeature()`).
- Add the route to `static/js/app.js` in the `routes` map.
- Update `static/index.html` to add any new nav links with `data-route="/my-feature"`.
- Use `navigate('/my-feature')` for programmatic navigation.

## 4. UI Best Practices

- **Tailwind v4**: Use utility classes for styling. Refer to `static/css/input.css` for custom theme variables.
- **Lucide Icons**: Use `lucide.createIcons()` after rendering new HTML.
- **Escaping**: Always use `escapeHtml` from `static/js/utils/html.js` for user-provided strings.
- **Offline Support**: Ensure the view handles `navigator.onLine` and `db.sync_queue` status (already integrated in the app shell).

## 5. Sync Integration

- Call `Sync.queueAction(table, item, 'create'|'update'|'delete')` for mutations.
- Call `Sync.pushChanges()` to trigger an immediate sync if online.
