import { Spin } from 'antd';
import hljs from 'highlight.js';
import { marked, type Tokens } from 'marked';
import React, { useEffect, useMemo, useRef } from 'react';

interface DocContentProps {
  markdown: string;
  loading?: boolean;
  onLinkClick?: (href: string) => void;
}

const renderer = new marked.Renderer();

renderer.code = ({ text, lang }: Tokens.Code): string => {
  const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
  const highlighted = hljs.highlight(text, { language }).value;
  const escaped = encodeURIComponent(text);
  return `<div class="code-block-wrapper">
    <span class="code-lang">${lang || ''}</span>
    <button class="code-copy-btn" data-code="${escaped}">Copy</button>
    <pre><code class="hljs language-${language}">${highlighted}</code></pre>
  </div>`;
};

renderer.heading = ({ text, depth }: Tokens.Heading): string => {
  const id = text
    .toLowerCase()
    .replace(/<[^>]*>/g, '')
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-');
  return `<h${depth} id="${id}">${text}</h${depth}>`;
};

marked.use({ renderer, gfm: true, breaks: false });

const DocContent: React.FC<DocContentProps> = ({
  markdown,
  loading,
  onLinkClick,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;

      const copyBtn = target.closest('.code-copy-btn') as HTMLElement;
      if (copyBtn) {
        e.preventDefault();
        const code = decodeURIComponent(copyBtn.dataset.code || '');
        navigator.clipboard.writeText(code).then(() => {
          const original = copyBtn.textContent;
          copyBtn.textContent = 'Copied!';
          setTimeout(() => {
            copyBtn.textContent = original;
          }, 1500);
        });
        return;
      }

      const link = target.closest('a') as HTMLAnchorElement;
      if (link && onLinkClick) {
        const href = link.getAttribute('href') || '';
        if (href.endsWith('.md')) {
          e.preventDefault();
          onLinkClick(href);
        }
      }
    };

    containerRef.current.addEventListener('click', handleClick);
    return () => {
      containerRef.current?.removeEventListener('click', handleClick);
    };
  }, [onLinkClick]);

  const html = useMemo(() => {
    if (!markdown) return '';
    return marked.parse(markdown) as string;
  }, [markdown]);

  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: '80px 0' }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="doc-content-body"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
};

export default DocContent;
