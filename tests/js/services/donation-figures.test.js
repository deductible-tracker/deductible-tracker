import {
  parseAmount,
  normalizeDonationCategory,
  calculateDonationFigures,
} from '../../../static/js/services/donation-figures.js';

describe('donation-figures', () => {
  test('parseAmount returns finite number or null', () => {
    expect(parseAmount('12.34')).toBeCloseTo(12.34);
    expect(parseAmount('not-a-number')).toBeNull();
  });

  test('normalizeDonationCategory defaults to money for unknown values', () => {
    expect(normalizeDonationCategory('weird')).toBe('money');
  });

  test('calculateDonationFigures aggregates counts and amounts', () => {
    const items = [
      { amount: 10, category: 'money' },
      { amount: 20, category: 'money' },
      { amount: 0, category: 'noncash' },
    ];
    const res = calculateDonationFigures(items);
    expect(res.total.count).toBe(3);
    expect(res.total.amount).toBe(30);
  });
});
