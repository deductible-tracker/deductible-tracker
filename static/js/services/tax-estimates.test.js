import {
    calculateTaxEstimates,
    isLikelyQualifiedCharity,
    normalizeFilingStatus
} from './tax-estimates.js';

describe('tax-estimates', () => {
    test('normalizeFilingStatus handles unknown values', () => {
        expect(normalizeFilingStatus('single')).toBe('single');
        expect(normalizeFilingStatus('MARRIED_JOINT')).toBe('married_joint');
        expect(normalizeFilingStatus('unexpected')).toBe('single');
    });

    test('isLikelyQualifiedCharity evaluates deductibility text', () => {
        expect(isLikelyQualifiedCharity({ deductibility: 'Public Charity' })).toBe(true);
        expect(isLikelyQualifiedCharity({ deductibility: 'not deductible' })).toBe(true);
        expect(isLikelyQualifiedCharity({ deductibility: '' })).toBe(true);
        expect(isLikelyQualifiedCharity(null)).toBe(false);
    });

    test('calculateTaxEstimates applies receipts and profile limits', async () => {
        const donations = [
            { id: 'd1', year: 2026, category: 'money', amount: 1000, charity_id: 'c1' },
            { id: 'd2', year: 2026, category: 'items', amount: 500, charity_id: 'c1' },
            { id: 'd3', year: 2026, category: 'money', amount: 700, charity_id: 'c1' }
        ];
        const charities = [{ id: 'c1', deductibility: 'deductible' }];
        const receipts = [{ donation_id: 'd1' }, { donation_id: 'd2' }];
        const profile = {
            filing_status: 'single',
            itemize_deductions: false,
            marginal_tax_rate: 0.2,
            agi: 100000
        };

        const result = await calculateTaxEstimates(donations, charities, receipts, profile);

        expect(result.perDonation.get('d1')).toBeCloseTo(200);
        expect(result.perDonation.get('d2')).toBe(0);
        expect(result.perDonation.get('d3')).toBeUndefined();
        expect(result.totalEstimated).toBeCloseTo(200);
    });
});
