import { Anchor } from 'antd';
import React, { useMemo } from 'react';

interface TocItem {
  id: string;
  text: string;
  level: number;
}

interface DocTocProps {
  markdown: string;
}

const DocToc: React.FC<DocTocProps> = ({ markdown }) => {
  const headings = useMemo(() => {
    const items: TocItem[] = [];
    const regex = /^(#{2,3})\s+(.+)$/gm;
    let match;
    while ((match = regex.exec(markdown)) !== null) {
      const level = match[1].length;
      const text = match[2].replace(/[`*_~]/g, '').trim();
      const id = text
        .toLowerCase()
        .replace(/[^\w\s-]/g, '')
        .replace(/\s+/g, '-');
      items.push({ id, text, level });
    }
    return items;
  }, [markdown]);

  if (headings.length === 0) return null;

  const items = headings.map((h) => ({
    key: h.id,
    href: `#${h.id}`,
    title: h.text,
  }));

  return (
    <div style={{ position: 'sticky', top: 80 }}>
      <div
        style={{
          fontSize: 13,
          fontWeight: 600,
          marginBottom: 12,
          color: '#666',
        }}
      >
        ON THIS PAGE
      </div>
      <Anchor items={items} affix={false} offsetTop={80} targetOffset={80} />
    </div>
  );
};

export default DocToc;
