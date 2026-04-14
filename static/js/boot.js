function renderBootError(message) {
  const app = document.getElementById('app');
  if (!app) return;

  app.innerHTML = `
    <div class="mx-auto max-w-2xl p-8 text-center">
      <h1 class="font-semibold text-rose-600 dark:text-rose-400">Application failed to load</h1>
      <p class="mt-2 text-sm text-slate-600 dark:text-slate-300">${message}</p>
      <button onclick="location.reload()" class="dt-btn-primary mt-4">Retry</button>
    </div>
  `;
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[data-dt-src="${src}"]`);
    if (existing) {
      if (existing.dataset.loaded === 'true') {
        resolve();
        return;
      }
      existing.addEventListener('load', () => resolve(), { once: true });
      existing.addEventListener('error', () => reject(new Error(`Failed to load ${src}`)), {
        once: true,
      });
      return;
    }

    const script = document.createElement('script');
    script.src = src;
    script.async = true;
    script.defer = true;
    script.dataset.dtSrc = src;
    script.onload = () => {
      script.dataset.loaded = 'true';
      resolve();
    };
    script.onerror = () => reject(new Error(`Failed to load ${src}`));
    document.head.appendChild(script);
  });
}

async function bootstrap() {
  const encodedBootstrap = document.body?.dataset?.dtBootstrap || '';
  const boot = encodedBootstrap ? JSON.parse(atob(encodedBootstrap)) : {};

  if ('serviceWorker' in navigator) {
    const swVersion = encodeURIComponent(boot.serviceWorkerVersion || 'dev');
    navigator.serviceWorker.register(`/sw.js?v=${swVersion}`).catch((error) => {
      console.warn('SW registration failed', error);
    });
  }

  if (!window.Dexie) {
    if (!boot.dexie) {
      throw new Error('Dexie asset URL is missing from the bootstrap payload.');
    }
    await loadScript(boot.dexie);
  }

  await import('./app.js');
}

bootstrap().catch((error) => {
  console.error('Boot failed', error);
  renderBootError(error && error.message ? error.message : 'Unknown boot error');
});
