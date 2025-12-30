import type { RequestOptions } from '@@/plugin-request/request';
import type { RequestConfig } from '@umijs/max';
import { message, notification } from 'antd';

// 错误处理方案： 错误类型
enum ErrorShowType {
  SILENT = 0,
  WARN_MESSAGE = 1,
  ERROR_MESSAGE = 2,
  NOTIFICATION = 3,
  REDIRECT = 9,
}
// 与后端约定的响应数据格式
interface ResponseStructure {
  success: boolean;
  data: any;
  errorCode?: number;
  errorMessage?: string;
  showType?: ErrorShowType;
}

/**
 * @name 错误处理
 * pro 自带的错误处理， 可以在这里做自己的改动
 * @doc https://umijs.org/docs/max/request#配置
 */
export const errorConfig: RequestConfig = {
  // 错误处理： umi@3 的错误处理方案。
  errorConfig: {
    // 错误抛出
    errorThrower: (res) => {
      const { success, data, errorCode, errorMessage, showType } =
        res as unknown as ResponseStructure;
      if (!success) {
        const error: any = new Error(errorMessage);
        error.name = 'BizError';
        error.info = { errorCode, errorMessage, showType, data };
        throw error; // 抛出自制的错误
      }
    },
    // 错误接收及处理
    errorHandler: (error: any, opts: any) => {
      if (opts?.skipErrorHandler) throw error;
      // 我们的 errorThrower 抛出的错误。
      if (error.name === 'BizError') {
        const errorInfo: ResponseStructure | undefined = error.info;
        if (errorInfo) {
          const { errorMessage, errorCode } = errorInfo;
          switch (errorInfo.showType) {
            case ErrorShowType.SILENT:
              // do nothing
              break;
            case ErrorShowType.WARN_MESSAGE:
              message.warning(errorMessage);
              break;
            case ErrorShowType.ERROR_MESSAGE:
              message.error(errorMessage);
              break;
            case ErrorShowType.NOTIFICATION:
              notification.open({
                title: errorCode,
                description: errorMessage,
              });
              break;
            case ErrorShowType.REDIRECT:
              // TODO: redirect
              break;
            default:
              message.error(errorMessage);
          }
        }
      } else if (error.response) {
        // Axios 的错误
        // 请求成功发出且服务器也响应了状态码，但状态代码超出了 2xx 的范围
        const status = error.response.status;
        let errorMsg = `请求失败 (${status})`;

        switch (status) {
          case 400:
            errorMsg = '请求参数错误，请检查输入';
            break;
          case 401:
            errorMsg = '未授权，请重新登录';
            break;
          case 403:
            errorMsg = '权限不足';
            break;
          case 404:
            errorMsg = '请求的资源不存在';
            break;
          case 429:
            errorMsg = '请求过于频繁，请稍后再试';
            break;
          case 500:
            errorMsg = '服务器内部错误，请稍后重试';
            break;
          case 503:
            errorMsg = '服务暂时不可用，请稍后重试';
            break;
          default:
            errorMsg = `请求失败 (${status})`;
        }

        message.error(errorMsg);
      } else if (error.request) {
        // 请求已经成功发起，但没有收到响应
        // \`error.request\` 在浏览器中是 XMLHttpRequest 的实例，
        // 而在node.js中是 http.ClientRequest 的实例
        message.error('网络错误，无法连接到服务器，请检查网络连接');
      } else {
        // 发送请求时出了点问题
        message.error('请求错误，请重试');
      }
    },
  },

  // 请求拦截器
  requestInterceptors: [
    (config: RequestOptions) => {
      // 拦截请求配置，进行个性化处理。
      // 移除自动添加 token 的逻辑，登录接口不需要在 URL 中添加 token
      // 确保跨域请求时发送 cookie
      config.withCredentials = true;

      // 从 localStorage 读取 token 并添加到 Authorization header
      // 作为 cookie 的备选方案
      const token = localStorage.getItem('jwt_token');
      if (token && config.headers) {
        // 设置 Authorization header
        config.headers['Authorization'] = `Bearer ${token}`;
      }

      return config;
    },
  ],

  // 响应拦截器
  responseInterceptors: [
    (response) => {
      // 拦截响应数据，进行个性化处理
      // Umi 的 request 会自动从 response.data 中提取数据
      // 如果后端直接返回数据（没有包装），response.data 就是实际数据
      // 如果后端返回 { success: true, data: ... }，response.data 就是 { success: true, data: ... }
      const { data } = response as any;

      // 如果响应格式是 { success: false, ... }，显示错误
      if (data && typeof data === 'object' && data.success === false) {
        const errorMessage = data?.errorMessage || data?.message || '请求失败';
        message.error(errorMessage);
      }

      // 如果响应格式是 { data: ... }，Umi 会自动提取 data 字段
      // 如果响应直接是数据，直接返回
      return response;
    },
  ],
};
