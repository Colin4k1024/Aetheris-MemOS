import { request } from '@umijs/max';

export interface LoginParams {
  username?: string;
  password?: string;
  captcha?: string;
  mobile?: string;
  autoLogin?: boolean;
  type?: string;
}

export interface LoginResult {
  success?: boolean;
  token?: string;
  user?: API.CurrentUser;
  errorMessage?: string;
}

/**
 * 用户登录
 */
export async function login(params: LoginParams): Promise<LoginResult> {
  return request<LoginResult>('/api/v1/auth/login', {
    method: 'POST',
    data: params,
  });
}

/**
 * 用户登出 (前端清理 + 通知后端)
 */
export async function outLogin(): Promise<void> {
  try {
    await request<void>('/api/v1/auth/logout', {
      method: 'POST',
      skipErrorHandler: true,
    });
  } finally {
    localStorage.removeItem('jwt_token');
  }
}

/**
 * 获取当前登录用户信息
 */
export async function currentUser(options?: {
  [key: string]: unknown;
}): Promise<{ data?: API.CurrentUser } | API.CurrentUser> {
  return request('/api/v1/user/me', {
    method: 'GET',
    ...(options || {}),
  });
}

/**
 * 获取短信/图片验证码
 */
export async function getFakeCaptcha(
  params: { phone?: string },
): Promise<string> {
  return request<string>('/api/v1/auth/captcha', {
    method: 'GET',
    params,
  });
}
