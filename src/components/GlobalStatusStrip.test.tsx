import { render, screen } from '@testing-library/react';
import { describe, test } from 'vitest';
import { GlobalStatusStrip } from './GlobalStatusStrip';
import type { ProviderSnapshot } from '../types';
import { I18nProvider } from '../i18n';
import type { ReactElement } from 'react';

const provider = (status: ProviderSnapshot['status']): ProviderSnapshot => ({
  id: 'test',
  name: 'Test Provider',
  status,
  source: 'remote',
  updatedAt: '2026-06-08T00:00:00.000Z',
  windows: [
    {
      id: 'weekly',
      label: 'Weekly limit',
      used: 1,
      limit: 10,
      unit: 'requests',
      usedPercent: 10,
      remainingPercent: 90,
      resetAt: null,
      resetText: null,
      confidence: 'exact'
    }
  ],
  error: status === 'stale' ? 'Connection timed out' : status === 'error' ? 'Refresh failed' : null,
  diagnostics: null,
  metadata: null
});

function renderWithEnglish(ui: ReactElement) {
  return render(ui, {
    wrapper: ({ children }) => (
      <I18nProvider language="en">{children}</I18nProvider>
    )
  });
}

describe('GlobalStatusStrip', () => {
  test('renders_stale_status_when_provider_is_stale', () => {
    renderWithEnglish(<GlobalStatusStrip providers={[provider('stale')]} />);
    expect(screen.getByText(/Test Provider data is stale/)).toBeInTheDocument();
  });

  test('prioritizes_error_over_stale', () => {
    renderWithEnglish(
      <GlobalStatusStrip providers={[provider('error'), provider('stale')]} />
    );
    expect(screen.getByText(/Test Provider refresh failed/)).toBeInTheDocument();
  });

  test('shows_stale_when_no_error_but_stale_exists', () => {
    renderWithEnglish(
      <GlobalStatusStrip providers={[provider('ok'), provider('stale')]} />
    );
    expect(screen.getByText(/Test Provider data is stale/)).toBeInTheDocument();
  });
});
