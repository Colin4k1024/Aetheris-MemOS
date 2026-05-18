import { FileTextOutlined, SearchOutlined } from '@ant-design/icons';
import { Input, List, Typography } from 'antd';
import React, { useMemo, useState } from 'react';
import { docsManifest } from '../docsManifest';

const { Text } = Typography;

interface DocSearchProps {
  onSelect: (docId: string) => void;
  locale?: string;
}

const DocSearch: React.FC<DocSearchProps> = ({
  onSelect,
  locale = 'zh-CN',
}) => {
  const [query, setQuery] = useState('');
  const isZh = locale.startsWith('zh');

  const allDocs = useMemo(() => {
    return docsManifest.flatMap((cat) =>
      cat.children.map((doc) => ({
        ...doc,
        categoryTitle: isZh ? cat.titleZh : cat.title,
      })),
    );
  }, [isZh]);

  const results = useMemo(() => {
    if (!query.trim()) return [];
    const q = query.toLowerCase();
    return allDocs.filter(
      (doc) =>
        doc.title.toLowerCase().includes(q) ||
        doc.titleZh.toLowerCase().includes(q) ||
        doc.path.toLowerCase().includes(q),
    );
  }, [query, allDocs]);

  return (
    <div style={{ padding: '8px 16px' }}>
      <Input
        prefix={<SearchOutlined />}
        placeholder={isZh ? '搜索文档...' : 'Search docs...'}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        allowClear
        size="small"
      />
      {results.length > 0 && (
        <List
          size="small"
          style={{
            marginTop: 8,
            maxHeight: 300,
            overflow: 'auto',
            border: '1px solid #f0f0f0',
            borderRadius: 4,
          }}
          dataSource={results}
          renderItem={(item) => (
            <List.Item
              style={{ cursor: 'pointer', padding: '6px 12px' }}
              onClick={() => {
                onSelect(item.id);
                setQuery('');
              }}
            >
              <FileTextOutlined style={{ marginRight: 8, color: '#999' }} />
              <Text style={{ flex: 1 }}>
                {isZh ? item.titleZh : item.title}
              </Text>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {item.categoryTitle}
              </Text>
            </List.Item>
          )}
        />
      )}
    </div>
  );
};

export default DocSearch;
