import { CopyOutlined } from '@ant-design/icons';
import { message } from 'antd';
import React, { useCallback } from 'react';

interface CodeBlockProps {
  className?: string;
  children?: React.ReactNode;
}

const CodeBlock: React.FC<CodeBlockProps> = ({ className, children }) => {
  const language = className?.replace('language-', '') || '';
  const code = String(children).replace(/\n$/, '');

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      message.success('Copied!');
    });
  }, [code]);

  return (
    <div style={{ position: 'relative', marginBottom: 16 }}>
      {language && (
        <span
          style={{
            position: 'absolute',
            top: 8,
            left: 12,
            fontSize: 11,
            color: '#999',
            textTransform: 'uppercase',
            userSelect: 'none',
          }}
        >
          {language}
        </span>
      )}
      <button
        onClick={handleCopy}
        style={{
          position: 'absolute',
          top: 8,
          right: 8,
          background: 'transparent',
          border: '1px solid #d9d9d9',
          borderRadius: 4,
          padding: '2px 6px',
          cursor: 'pointer',
          color: '#999',
          fontSize: 12,
          display: 'flex',
          alignItems: 'center',
          gap: 4,
        }}
      >
        <CopyOutlined /> Copy
      </button>
      <pre
        style={{
          padding: '32px 12px 12px',
          borderRadius: 8,
          overflow: 'auto',
          background: '#f6f8fa',
          border: '1px solid #e8e8e8',
        }}
      >
        <code className={className}>{children}</code>
      </pre>
    </div>
  );
};

export default CodeBlock;
