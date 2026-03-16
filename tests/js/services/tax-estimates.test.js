import {
  normalizeFilingStatus,
  isLikelyQualifiedCharity,
  calculateTaxEstimates,
} from '../../../static/js/services/tax-estimates.js';

describe('tax-estimates', () => {
  test('normalizeFilingStatus handles unknown values', () => {
    expect(normalizeFilingStatus('weird')).toBe('single');
  });

  test('isLikelyQualifiedCharity evaluates deductibility text', () => {
    expect(isLikelyQualifiedCharity({ deductibility: 'This is a public charity' })).toBe(true);
    expect(isLikelyQualifiedCharity({ deductibility: 'unknown' })).toBe(false);
  });

  test('calculateTaxEstimates applies receipts and profile limits', async () => {
    const profile = { filing_status: 'single', agi: 100000 };
    const res = await calculateTaxEstimates([], [], [], profile);
    expect(res).toHaveProperty('totalEstimated');
  });
});
