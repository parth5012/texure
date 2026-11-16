import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ErrorBoundary } from '../components/ErrorBoundary';

const BombComponent: React.FC<{ shouldThrow?: boolean }> = ({ shouldThrow }) => {
  if (shouldThrow) {
    throw new Error('Explosion in child component');
  }
  return <div>Healthy Component Content</div>;
};

describe('ErrorBoundary Component', () => {
  it('renders children when no error occurs', () => {
    render(
      <ErrorBoundary>
        <BombComponent shouldThrow={false} />
      </ErrorBoundary>
    );

    expect(screen.getByText('Healthy Component Content')).toBeInTheDocument();
  });

  it('catches render error and renders fallback UI with retry button', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const { rerender } = render(
      <ErrorBoundary fallbackTitle="Tab Error">
        <BombComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByTestId('error-boundary-fallback')).toBeInTheDocument();
    expect(screen.getByText('Tab Error')).toBeInTheDocument();
    expect(screen.getByText(/Explosion in child component/i)).toBeInTheDocument();

    // Recover by re-rendering with non-throwing child and clicking retry
    rerender(
      <ErrorBoundary fallbackTitle="Tab Error">
        <BombComponent shouldThrow={false} />
      </ErrorBoundary>
    );

    const retryBtn = screen.getByRole('button', { name: /Retry Tab/i });
    fireEvent.click(retryBtn);

    expect(screen.getByText('Healthy Component Content')).toBeInTheDocument();
    consoleSpy.mockRestore();
  });
});
