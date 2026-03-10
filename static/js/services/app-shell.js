export function captureAppShellTemplate(appElement) {
  if (!appElement) return '';
  return appElement.innerHTML || '';
}

export function restoreAppShellIfMissingRouteContent(appElement, shellTemplate) {
  if (!appElement || !shellTemplate) return false;
  const hasRouteContent = !!appElement.querySelector('#route-content');
  if (hasRouteContent) return false;
  appElement.innerHTML = shellTemplate;
  return true;
}
