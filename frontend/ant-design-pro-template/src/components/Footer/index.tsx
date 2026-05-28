import { GithubOutlined } from '@ant-design/icons';
import { DefaultFooter } from '@ant-design/pro-components';
import React from 'react';

const Footer: React.FC = () => {
  return (
    <DefaultFooter
      style={{
        background: 'none',
      }}
      copyright={`© ${new Date().getFullYear()} Aetheris MemOS`}
      links={[
        {
          key: 'docs',
          title: 'Documentation',
          href: 'https://github.com/Colin4k1024/Aetheris-MemOS#readme',
          blankTarget: true,
        },
        {
          key: 'github',
          title: <GithubOutlined />,
          href: 'https://github.com/Colin4k1024/Aetheris-MemOS',
          blankTarget: true,
        },
      ]}
    />
  );
};

export default Footer;
