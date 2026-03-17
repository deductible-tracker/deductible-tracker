import {
  mapReceiptSuggestionToDonationDraft,
  normalizeReceiptAnalysis,
} from '../../../static/js/services/receipt-upload.js';

describe('receipt upload helpers', () => {
  test('normalizeReceiptAnalysis reshapes API payloads into frontend-friendly data', () => {
    const normalized = normalizeReceiptAnalysis({
      status: 'done',
      warning: 'Mistral enrichment is not configured; only OCR text was extracted.',
      ocr_text: 'Sample receipt text',
      ocr_date: '2026-03-17',
      ocr_amount_usd: 50,
      suggestion: {
        date_of_donation: '2026-03-17',
        organization_name: 'American Red Cross',
        donation_type: 'money',
        item_name: null,
        amount_usd: 50,
      },
    });

    expect(normalized).toEqual({
      status: 'done',
      warning: 'Mistral enrichment is not configured; only OCR text was extracted.',
      ocrText: 'Sample receipt text',
      ocrDate: '2026-03-17',
      ocrAmountUsd: 50,
      suggestion: {
        dateOfDonation: '2026-03-17',
        organizationName: 'American Red Cross',
        donationType: 'money',
        itemName: null,
        amountUsd: 50,
      },
    });
  });

  test('mapReceiptSuggestionToDonationDraft maps item suggestions to donation form fields', () => {
    const patch = mapReceiptSuggestionToDonationDraft({
      suggestion: {
        dateOfDonation: '2026-03-01',
        organizationName: 'Local Food Bank',
        donationType: 'item',
        itemName: 'Winter Coat',
        amountUsd: null,
      },
    });

    expect(patch).toEqual({
      date: '2026-03-01',
      charityName: 'Local Food Bank',
      category: 'items',
      itemName: 'Winter Coat',
      amount: null,
    });
  });

  test('mapReceiptSuggestionToDonationDraft returns null when suggestion data is missing', () => {
    expect(mapReceiptSuggestionToDonationDraft(null)).toBeNull();
    expect(mapReceiptSuggestionToDonationDraft({ suggestion: null })).toBeNull();
  });
});