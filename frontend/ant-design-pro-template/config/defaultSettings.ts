import type { ProLayoutProps } from '@ant-design/pro-components';

/**
 * @name Aetheris-MemOS layout settings
 */
const Settings: ProLayoutProps & {
  pwa?: boolean;
  logo?: string;
} = {
  navTheme: 'realDark',
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
  token: {},
};

export default Settings;
