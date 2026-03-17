const ICONS = {
  calculator:
    '<rect x="4" y="2" width="16" height="20" rx="2"></rect><line x1="8" y1="6" x2="16" y2="6"></line><line x1="16" y1="14" x2="16" y2="18"></line><path d="M16 10h.01"></path><path d="M12 10h.01"></path><path d="M8 10h.01"></path><path d="M12 14h.01"></path><path d="M8 14h.01"></path><path d="M12 18h.01"></path><path d="M8 18h.01"></path>',
  loader: '<path d="M21 12a9 9 0 1 1-6.22-8.56"></path>',
  cloud: '<path d="M17.5 19H9a7 7 0 1 1 1.67-13.8A5.5 5.5 0 1 1 17.5 19Z"></path>',
  'cloud-off':
    '<path d="M16 16.58A5 5 0 0 0 9.86 9.14"></path><path d="M5.31 11.67A7 7 0 0 0 9 19h9a5 5 0 0 0 .58-9.97"></path><path d="m2 2 20 20"></path>',
  download: '<path d="M12 15V3"></path><path d="m7 10 5 5 5-5"></path><path d="M5 21h14"></path>',
  upload: '<path d="M12 3v12"></path><path d="m17 8-5-5-5 5"></path><path d="M5 21h14"></path>',
  calendar:
    '<path d="M8 2v4"></path><path d="M16 2v4"></path><rect width="18" height="18" x="3" y="4" rx="2"></rect><path d="M3 10h18"></path>',
};

function escapeAttribute(value) {
  return String(value || '')
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

export function iconSvg(name, className = '') {
  const markup = ICONS[name];
  if (!markup) return '';

  const safeClassName = escapeAttribute(className);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" class="${safeClassName}">${markup}</svg>`;
}
