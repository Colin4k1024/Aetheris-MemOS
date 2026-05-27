import {
  ArrowLeftOutlined,
  DashboardOutlined,
  GithubOutlined,
  LoginOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { history, useIntl } from '@umijs/max';
import { Button } from 'antd';
import React, { useCallback, useEffect, useState } from 'react';
import DocContent from './components/DocContent';
import DocSearch from './components/DocSearch';
import DocSidebar from './components/DocSidebar';
import DocToc from './components/DocToc';
import { docsManifest, findDocById, getDefaultDocId } from './docsManifest';
import useStyles from './style';
import 'highlight.js/styles/github.css';
import './docContent.css';

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
        const resp = await fetch(
          new URL(`docs/${filePath}`, document.baseURI).href,
        );
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
    <div
      style={{ minHeight: '100vh', display: 'flex', flexDirection: 'column' }}
    >
      <header className={styles.header}>
        <div className={styles.headerLeft}>
          <a className={styles.logo} onClick={() => history.push('/home')}>
            <ThunderboltOutlined /> Aetheris-MemOS
          </a>
          <span className={styles.headerDivider} />
          <span className={styles.headerTitle}>Documentation</span>
        </div>
        <nav className={styles.headerNav}>
          <a onClick={() => history.push('/home')}>
            <ArrowLeftOutlined /> Home
          </a>
          <a onClick={() => history.push('/dashboard')}>
            <DashboardOutlined /> Dashboard
          </a>
          <a
            href="https://github.com/Colin4k1024/Aetheris-MemOS"
            target="_blank"
            rel="noopener noreferrer"
          >
            <GithubOutlined /> GitHub
          </a>
          <Button
            type="primary"
            size="small"
            icon={<LoginOutlined />}
            onClick={() => history.push('/user/login')}
          >
            Login
          </Button>
        </nav>
      </header>
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
    </div>
  );
};

export default DocumentationPage;
