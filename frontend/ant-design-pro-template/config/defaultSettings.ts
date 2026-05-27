import type { ProLayoutProps } from '@ant-design/pro-components';

const Settings: ProLayoutProps & {
  pwa?: boolean;
  logo?: string;
} = {
  navTheme: 'light',
  colorPrimary: '#6366f1',
  layout: 'side',
  contentWidth: 'Fluid',
  fixedHeader: true,
  fixSiderbar: true,
  colorWeak: false,
  title: 'Aetheris-MemOS',
  pwa: true,
  logo: '/logo.svg',
  iconfontUrl: '',
  token: {
    bgLayout: '#f8fafc',
    sider: {
      colorMenuBackground: '#1e1b4b',
      colorTextMenu: 'rgba(255,255,255,0.75)',
      colorTextMenuSelected: '#ffffff',
      colorBgMenuItemSelected: 'rgba(99,102,241,0.25)',
      colorTextMenuItemHover: '#ffffff',
      colorBgMenuItemHover: 'rgba(99,102,241,0.12)',
      colorTextMenuTitle: '#ffffff',
      colorMenuItemDivider: 'rgba(255,255,255,0.06)',
    },
    header: {
      colorBgHeader: '#ffffff',
      colorHeaderTitle: '#1e1b4b',
      colorTextMenu: '#64748b',
      colorTextMenuSelected: '#6366f1',
      colorBgMenuItemSelected: 'rgba(99,102,241,0.08)',
      heightLayoutHeader: 56,
    },
    pageContainer: {
      paddingBlockPageContainerContent: 24,
      paddingInlinePageContainerContent: 24,
    },
  },
};

export default Settings;
