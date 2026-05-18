import { PageContainer } from '@ant-design/pro-components';
import { useIntl } from '@umijs/max';
import React, { useCallback, useEffect, useState } from 'react';
import DocContent from './components/DocContent';
import DocSearch from './components/DocSearch';
import DocSidebar from './components/DocSidebar';
import DocToc from './components/DocToc';
import { docsManifest, findDocById, getDefaultDocId } from './docsManifest';
import useStyles from './style';
import 'highlight.js/styles/github.css';

const DocumentationPage: React.FC = () => {
  const { styles } = useStyles();
  const intl = useIntl();
  const locale = intl.locale || 'zh-CN';
  const isZh = locale.startsWith('zh');

  const [activeDocId, setActiveDocId] = useState(getDefaultDocId());
  const [markdown, setMarkdown] = useState('');
  const [loading, setLoading] = useState(false);

  const loadDoc = useCallback(
    async (docId: string) => {
      const doc = findDocById(docId);
      if (!doc) return;

      const filePath = isZh && doc.pathZh ? doc.pathZh : doc.path;
      setLoading(true);
      try {
        const resp = await fetch(`/docs/${filePath}`);
        if (resp.ok) {
          const text = await resp.text();
          setMarkdown(text);
        } else {
          setMarkdown(
            `# Document not found\n\nCould not load \`${filePath}\`.`,
          );
        }
      } catch {
        setMarkdown(`# Load Error\n\nFailed to fetch document.`);
      } finally {
        setLoading(false);
      }
    },
    [isZh],
  );

  useEffect(() => {
    loadDoc(activeDocId);
  }, [activeDocId, loadDoc]);

  const handleDocSelect = (docId: string) => {
    setActiveDocId(docId);
    const contentEl = document.querySelector(`.${styles.content}`);
    if (contentEl) contentEl.scrollTop = 0;
  };

  const handleLinkClick = (href: string) => {
    const filename = href.split('/').pop() || '';
    for (const cat of docsManifest) {
      const found = cat.children.find(
        (d) => d.path === filename || d.pathZh === filename,
      );
      if (found) {
        handleDocSelect(found.id);
        return;
      }
    }
  };

  const currentDoc = findDocById(activeDocId);
  const title = currentDoc
    ? isZh
      ? currentDoc.titleZh
      : currentDoc.title
    : '';

  return (
    <PageContainer
      header={{ title: '', breadcrumb: {} }}
      childrenContentStyle={{ padding: 0 }}
    >
      <div className={styles.container}>
        <div className={styles.sidebar}>
          <DocSearch onSelect={handleDocSelect} locale={locale} />
          <DocSidebar
            activeDocId={activeDocId}
            onSelect={handleDocSelect}
            locale={locale}
          />
        </div>
        <div className={styles.content}>
          <div className={styles.docTitle}>{title}</div>
          <div className={styles.markdownBody}>
            <DocContent
              markdown={markdown}
              loading={loading}
              onLinkClick={handleLinkClick}
            />
          </div>
        </div>
        <div className={styles.toc}>
          <DocToc markdown={markdown} />
        </div>
      </div>
    </PageContainer>
  );
};

export default DocumentationPage;
