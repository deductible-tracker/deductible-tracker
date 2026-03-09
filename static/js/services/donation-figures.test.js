import {
    calculateDonationFigures,
    formatCurrency,
    formatFigureText,
    normalizeDonationCategory,
    parseAmount
} from './donation-figures.js';

describe('donation-figures', () => {
    test('parseAmount returns finite number or null', () => {
        expect(parseAmount('12.34')).toBe(12.34);
        expect(parseAmount('abc')).toBeNull();
    });

    test('normalizeDonationCategory defaults to money for unknown values', () => {
        expect(normalizeDonationCategory('items')).toBe('items');
        expect(normalizeDonationCategory('MILEAGE')).toBe('mileage');
        expect(normalizeDonationCategory('other')).toBe('money');
    });

    test('calculateDonationFigures aggregates counts and amounts', () => {
        const figures = calculateDonationFigures([
            { category: 'money', amount: '10.5' },
            { category: 'items', amount: 20 },
            { category: 'mileage', amount: 'not-number' }
        ]);

        expect(figures.total.count).toBe(3);
        expect(figures.total.amount).toBeCloseTo(30.5);
        expect(figures.money.amount).toBeCloseTo(10.5);
        expect(figures.items.amount).toBeCloseTo(20);
        expect(figures.mileage.count).toBe(1);
    });

    test('formatters render expected text', () => {
        expect(formatFigureText({ amount: 15.2, count: 1, hasAmount: true })).toBe('$15.20');
        expect(formatFigureText({ amount: 1234.5, count: 1, hasAmount: true })).toBe('$1,234.50');
        expect(formatFigureText({ amount: 0, count: 2, hasAmount: false })).toBe('2');
        expect(formatCurrency(19.995)).toBe('$20.00');
        expect(formatCurrency(1234567.89)).toBe('$1,234,567.89');
    });
});
