import { Spin } from 'antd';
import React from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeSlug from 'rehype-slug';
import remarkGfm from 'remark-gfm';
import CodeBlock from './CodeBlock';

interface DocContentProps {
  markdown: string;
  loading?: boolean;
  onLinkClick?: (href: string) => void;
}

const DocContent: React.FC<DocContentProps> = ({
  markdown,
  loading,
  onLinkClick,
}) => {
  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: '80px 0' }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <div className="doc-content-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight, rehypeSlug]}
        components={{
          code({ className, children, ...props }) {
            const isInline = !className;
            if (isInline) {
              return (
                <code
                  style={{
                    background: '#f0f0f0',
                    padding: '2px 6px',
                    borderRadius: 4,
                    fontSize: '0.9em',
                  }}
                  {...props}
                >
                  {children}
                </code>
              );
            }
            return <CodeBlock className={className}>{children}</CodeBlock>;
          },
          a({ href, children, ...props }) {
            if (href && href.endsWith('.md') && onLinkClick) {
              return (
                <a
                  {...props}
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    onLinkClick(href);
                  }}
                >
                  {children}
                </a>
              );
            }
            return (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                {...props}
              >
                {children}
              </a>
            );
          },
          table({ children, ...props }) {
            return (
              <div style={{ overflowX: 'auto', marginBottom: 16 }}>
                <table
                  style={{
                    borderCollapse: 'collapse',
                    width: '100%',
                    fontSize: 14,
                  }}
                  {...props}
                >
                  {children}
                </table>
              </div>
            );
          },
          th({ children, ...props }) {
            return (
              <th
                style={{
                  border: '1px solid #e8e8e8',
                  padding: '8px 12px',
                  background: '#fafafa',
                  fontWeight: 600,
                  textAlign: 'left',
                }}
                {...props}
              >
                {children}
              </th>
            );
          },
          td({ children, ...props }) {
            return (
              <td
                style={{
                  border: '1px solid #e8e8e8',
                  padding: '8px 12px',
                }}
                {...props}
              >
                {children}
              </td>
            );
          },
        }}
      >
        {markdown}
      </ReactMarkdown>
    </div>
  );
};

export default DocContent;
