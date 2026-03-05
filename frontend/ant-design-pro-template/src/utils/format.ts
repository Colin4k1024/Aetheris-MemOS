/**
 * Common utility functions for formatting data
 */

/**
 * Format timestamp to time string (HH:mm)
 */
export const formatTime = (timestamp: number | string): string => {
  const date = new Date(parseInt(String(timestamp)));
  return `${date.getHours().toString().padStart(2, '0')}:${date
    .getMinutes()
    .toString()
    .padStart(2, '0')}`;
};

/**
 * Format timestamp to datetime string (YYYY-MM-DD HH:mm:ss)
 */
export const formatDateTime = (timestamp: number | string): string => {
  const date = new Date(parseInt(String(timestamp)));
  return `${date.getFullYear()}-${(date.getMonth() + 1)
    .toString()
    .padStart(2, '0')}-${date.getDate().toString().padStart(2, '0')} ${date
  .getHours()
  .toString()
  .padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}:${date
  .getSeconds()
  .toString()
  .padStart(2, '0')}`;
};

/**
 * Format timestamp to date string (YYYY-MM-DD)
 */
export const formatDate = (timestamp: number | string): string => {
  const date = new Date(parseInt(String(timestamp)));
  return `${date.getFullYear()}-${(date.getMonth() + 1)
    .toString()
    .padStart(2, '0')}-${date.getDate().toString().padStart(2, '0')}`;
};

/**
 * Format number to percentage
 */
export const formatPercent = (value: number, decimals: number = 1): string => {
  return `${(value * 100).toFixed(decimals)}%`;
};

/**
 * Format bytes to human readable string
 */
export const formatBytes = (bytes: number, decimals: number = 2): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(decimals))} ${sizes[i]}`;
};

/**
 * Format duration in milliseconds to human readable string
 */
export const formatDuration = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  return `${Math.floor(ms / 3600000)}h ${Math.floor((ms % 3600000) / 60000)}m`;
};

/**
 * Format number with thousands separator
 */
export const formatNumber = (num: number): string => {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
};

/**
 * Truncate string with ellipsis
 */
export const truncate = (str: string, maxLength: number): string => {
  if (str.length <= maxLength) return str;
  return `${str.slice(0, maxLength)}...`;
};

/**
 * Capitalize first letter
 */
export const capitalize = (str: string): string => {
  return str.charAt(0).toUpperCase() + str.slice(1);
};
