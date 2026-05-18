import {
  ApiOutlined,
  ClusterOutlined,
  ExperimentOutlined,
  LinkOutlined,
  ProjectOutlined,
  RocketOutlined,
} from '@ant-design/icons';
import { Menu } from 'antd';
import React from 'react';
import { type DocCategory, docsManifest } from '../docsManifest';

const iconMap: Record<string, React.ReactNode> = {
  rocket: <RocketOutlined />,
  cluster: <ClusterOutlined />,
  api: <ApiOutlined />,
  experiment: <ExperimentOutlined />,
  link: <LinkOutlined />,
  project: <ProjectOutlined />,
};

interface DocSidebarProps {
  activeDocId: string;
  onSelect: (docId: string) => void;
  locale?: string;
}

const DocSidebar: React.FC<DocSidebarProps> = ({
  activeDocId,
  onSelect,
  locale = 'zh-CN',
}) => {
  const isZh = locale.startsWith('zh');

  const menuItems = docsManifest.map((category: DocCategory) => ({
    key: category.key,
    icon: iconMap[category.icon],
    label: isZh ? category.titleZh : category.title,
    children: category.children.map((doc) => ({
      key: doc.id,
      label: isZh ? doc.titleZh : doc.title,
    })),
  }));

  const openKeys = docsManifest
    .filter((cat) => cat.children.some((d) => d.id === activeDocId))
    .map((cat) => cat.key);

  return (
    <Menu
      mode="inline"
      selectedKeys={[activeDocId]}
      defaultOpenKeys={openKeys}
      items={menuItems}
      onClick={({ key }) => onSelect(key)}
      style={{ border: 'none', height: '100%' }}
    />
  );
};

export default DocSidebar;
