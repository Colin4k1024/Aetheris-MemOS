import { LinkOutlined, LoadingOutlined } from '@ant-design/icons';
import type { Settings as LayoutSettings } from '@ant-design/pro-components';
import { SettingDrawer } from '@ant-design/pro-components';
import type { RequestConfig, RunTimeLayoutConfig } from '@umijs/max';
import { history, Link } from '@umijs/max';
import { Spin } from 'antd';
import React from 'react';
import {
  AvatarDropdown,
  AvatarName,
  Footer,
  Question,
  SelectLang,
} from '@/components';
import ErrorBoundary from '@/components/ErrorBoundary';
import { currentUser as queryCurrentUser } from '@/services/memory/auth';
import defaultSettings from '../config/defaultSettings';
import { errorConfig } from './requestErrorConfig';

const isDev = process.env.NODE_ENV === 'development';
const isDevOrTest = isDev || process.env.CI;
const loginPath = '/user/login';

// Pages that don't require authentication
const PUBLIC_PATHS = [
  loginPath,
  '/',
  '/home',
  '/documentation',
  '/user/register',
];

/**
 * @see https://umijs.org/docs/api/runtime-config#getinitialstate
 * */
export async function getInitialState(): Promise<{
  settings?: Partial<LayoutSettings>;
  currentUser?: API.CurrentUser;
  loading?: boolean;
  fetchUserInfo?: () => Promise<API.CurrentUser | undefined>;
}> {
  const fetchUserInfo = async () => {
    try {
      const msg = await queryCurrentUser({
        skipErrorHandler: true,
      });
      // Backend may return { data: {...} } or directly { name, ... }
      if (msg && 'data' in msg && msg.data) {
        // Reject invalid user (e.g. {isLogin: false} from mock 401)
        if (
          (msg.data as API.CurrentUser).isLogin === false ||
          !(msg.data as API.CurrentUser).name
        ) {
          return undefined;
        }
        return msg.data as API.CurrentUser;
      }
      if (msg && 'name' in (msg as Record<string, unknown>)) {
        return msg as unknown as API.CurrentUser;
      }
      return undefined;
    } catch {
      return undefined;
    }
  };

  const { location } = history;
  // Only fetch user info for protected pages
  if (!PUBLIC_PATHS.includes(location.pathname)) {
    const currentUser = await fetchUserInfo();
    if (!currentUser) {
      // No valid session — redirect to login
      history.push(loginPath);
    }
    return {
      fetchUserInfo,
      currentUser,
      settings: defaultSettings as Partial<LayoutSettings>,
    };
  }
  return {
    fetchUserInfo,
    settings: defaultSettings as Partial<LayoutSettings>,
  };
}

// ProLayout 支持的api https://procomponents.ant.design/components/layout
export const layout: RunTimeLayoutConfig = ({
  initialState,
  setInitialState,
}) => {
  return {
    contentStyle: {
      padding: '24px',
    },
    actionsRender: () => [
      <Question key="doc" />,
      <SelectLang key="SelectLang" />,
    ],
    avatarProps: {
      src: initialState?.currentUser?.avatar,
      title: <AvatarName />,
      render: (_, avatarChildren) => (
        <AvatarDropdown>{avatarChildren}</AvatarDropdown>
      ),
    },
    waterMarkProps: {
      content: initialState?.currentUser?.name,
    },
    footerRender: () => <Footer />,
    onPageChange: () => {
      const { location } = history;
      // Redirect to login if not authenticated and not on public page
      if (
        !initialState?.currentUser &&
        !PUBLIC_PATHS.includes(location.pathname)
      ) {
        history.push(loginPath);
      }
    },
    links: isDevOrTest
      ? [
          <Link key="openapi" to="/umi/plugin/openapi" target="_blank">
            <LinkOutlined />
            <span>OpenAPI 文档</span>
          </Link>,
        ]
      : [],
    menuHeaderRender: undefined,
    childrenRender: (children) => {
      return (
        <>
          <ErrorBoundary>
            <React.Suspense
              fallback={
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'center',
                    alignItems: 'center',
                    height: '60vh',
                  }}
                >
                  <Spin indicator={<LoadingOutlined spin />} size="large" />
                </div>
              }
            >
              {children}
            </React.Suspense>
          </ErrorBoundary>
          {isDevOrTest && (
            <SettingDrawer
              disableUrlParams
              enableDarkTheme
              settings={initialState?.settings}
              onSettingChange={(settings) => {
                setInitialState((preInitialState) => ({
                  ...preInitialState,
                  settings,
                }));
              }}
            />
          )}
        </>
      );
    },
    ...initialState?.settings,
  };
};

/**
 * @name request 配置，可以配置错误处理
 * @doc https://umijs.org/docs/max/request#配置
 */
export const request: RequestConfig = {
  baseURL: isDev ? '' : '',
  timeout: 10000,
  ...errorConfig,
};
