import { captureAppShellTemplate, restoreAppShellIfMissingRouteContent } from './app-shell.js';

describe('app shell restore after login', () => {
    test('restores workspace overview shell when route content is missing', () => {
        const app = {
            innerHTML: '<div id="home-summary">Workspace Overview</div><section id="route-content"></section>',
            querySelector: () => null,
        };
        const shellTemplate = captureAppShellTemplate(app);

        app.innerHTML = '<div id="login-form">Sign in</div>';
        const restored = restoreAppShellIfMissingRouteContent(app, shellTemplate);

        expect(restored).toBe(true);
        expect(app.innerHTML).toContain('home-summary');
        expect(app.innerHTML).toContain('route-content');
    });

    test('does not overwrite shell when route content already exists', () => {
        const app = {
            innerHTML: '<section id="route-content"><p>Existing route</p></section>',
            querySelector: () => ({ id: 'route-content' }),
        };
        const original = app.innerHTML;
        const restored = restoreAppShellIfMissingRouteContent(app, '<div id="home-summary">X</div><section id="route-content"></section>');

        expect(restored).toBe(false);
        expect(app.innerHTML).toBe(original);
    });
});