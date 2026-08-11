import { render } from '@testing-library/react';
import React from 'react';
import DocContent from './DocContent';

// Behavior tests for DocContent's markdown sanitization. These pin the
// security-relevant contract implemented in the component's DOMPurify pipeline:
// reverse-tabnabbing hardening on `target="_blank"` links, and XSS stripping of
// scripts and inline event handlers. The test adapts to the component; it does
// not alter the sanitize configuration or the post-sanitize attribute hook.

describe('DocContent sanitization', () => {
  const getBody = (markdown: string): HTMLElement => {
    const { container } = render(<DocContent markdown={markdown} />);
    const body = container.querySelector('.doc-content-body');
    if (!body) {
      throw new Error('doc-content-body was not rendered');
    }
    return body as HTMLElement;
  };

  it('adds rel="noopener noreferrer" to target="_blank" links', () => {
    const body = getBody(
      '<a href="https://example.com" target="_blank">external</a>',
    );
    const link = body.querySelector('a');
    expect(link).not.toBeNull();
    expect(link?.getAttribute('target')).toBe('_blank');

    const rel = (link?.getAttribute('rel') ?? '').split(/\s+/).filter(Boolean);
    expect(rel).toContain('noopener');
    expect(rel).toContain('noreferrer');
  });

  it('preserves existing rel tokens while adding the hardening tokens', () => {
    const body = getBody(
      '<a href="https://example.com" target="_blank" rel="nofollow">external</a>',
    );
    const rel = (body.querySelector('a')?.getAttribute('rel') ?? '')
      .split(/\s+/)
      .filter(Boolean);
    expect(rel).toContain('nofollow');
    expect(rel).toContain('noopener');
    expect(rel).toContain('noreferrer');
  });

  it('does not inject rel into same-tab links', () => {
    const body = getBody('[home](https://example.com)');
    const link = body.querySelector('a');
    expect(link).not.toBeNull();
    expect(link?.getAttribute('target')).toBeNull();
    expect(link?.getAttribute('rel')).toBeNull();
  });

  it('strips <script> tags from rendered markdown', () => {
    const body = getBody(
      'safe text\n\n<script>window.__pwned = true;</script>',
    );
    expect(body.querySelector('script')).toBeNull();
    expect(body.textContent).toContain('safe text');
  });

  it('strips inline event handlers such as onerror', () => {
    const body = getBody('<img src="x" onerror="window.__pwned = true;">');
    const img = body.querySelector('img');
    expect(img).not.toBeNull();
    expect(img?.getAttribute('onerror')).toBeNull();
  });

  it('drops javascript: URIs so they never reach href', () => {
    const body = getBody('<a href="javascript:window.__pwned=1">click</a>');
    const href = body.querySelector('a')?.getAttribute('href') ?? '';
    expect(href).not.toContain('javascript:');
  });
});
