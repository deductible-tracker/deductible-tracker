/**
 * Donations route — re-exports from split modules.
 */
export { renderDonationsRoute } from './donations/list.js';
export { renderDonationViewRoute } from './donations/detail.js';
export {
  renderDonationNewRoute,
  renderDonationEditRoute,
} from './donations/form.js';

export async function renderReceiptPageRoute(_donationId, deps) {
  // This route is no longer used but we keep the export for now to avoid breaking imports.
  await deps.navigate('/donations');
}
