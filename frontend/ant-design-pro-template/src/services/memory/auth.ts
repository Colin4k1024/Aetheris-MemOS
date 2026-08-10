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
  return request<LoginResult>('/api/login', {
    method: 'POST',
    data: params,
  });
}

/**
 * 用户登出
 * NOTE: Backend has no logout endpoint. This is a no-op that only performs
 * client-side cleanup (localStorage). The backend does not need to be notified
 * because JWT tokens are stateless — the client simply discards the token.
 */
export async function outLogin(): Promise<void> {
  localStorage.removeItem('jwt_token');
}

/**
 * 获取当前登录用户信息
 */
export async function currentUser(options?: {
  [key: string]: unknown;
}): Promise<{ data?: API.CurrentUser } | API.CurrentUser> {
  return request('/api/currentUser', {
    method: 'GET',
    ...(options || {}),
  });
}

/**
 * 获取短信/图片验证码
 * NOTE: Backend has no captcha endpoint. This is a no-op that returns an empty
 * string so the login form's captcha field remains functional (never triggers
 * validation). The backend does not require captcha for authentication.
 */
export async function getFakeCaptcha(
  _params: { phone?: string },
): Promise<string> {
  return '';
}
