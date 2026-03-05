/**
 * Global configuration for Adaptive Memory System
 * This file contains default values and configuration constants
 */

// Default user and agent IDs for API requests
// In production, these should come from authentication
export const DEFAULT_USER_ID = 'user_1';
export const DEFAULT_AGENT_ID = 'agent_1';

// API Configuration
export const API_CONFIG = {
  // Backend API base URL
  BASE_URL: process.env.NODE_ENV === 'development' ? 'http://127.0.0.1:8008' : '',
  // Request timeout in ms
  TIMEOUT: 30000,
};

// Polling intervals (in milliseconds)
export const POLLING_INTERVALS = {
  // Fast polling for real-time data (e.g., resource monitoring)
  FAST: 3000,
  // Normal polling interval
  NORMAL: 5000,
  // Slow polling for less critical data
  SLOW: 10000,
  // Very slow polling for historical data
  VERY_SLOW: 30000,
};

// Chart colors
export const CHART_COLORS = {
  PRIMARY: '#1890ff',
  SUCCESS: '#52c41a',
  WARNING: '#faad14',
  ERROR: '#f5222d',
  INFO: '#13c2c2',
  TEXT: 'rgba(0, 0, 0, 0.65)',
  BORDER: '#d9d9d9',
  BACKGROUND: '#ffffff',
};

// Memory layer colors for visualization
export const MEMORY_LAYER_COLORS = {
  STM: '#1890ff', // Blue
  LTM: '#52c41a', // Green
  KG: '#722ed1',  // Purple
  MM: '#fa8c16',  // Orange
};

export default {
  DEFAULT_USER_ID,
  DEFAULT_AGENT_ID,
  API_CONFIG,
  POLLING_INTERVALS,
  CHART_COLORS,
  MEMORY_LAYER_COLORS,
};
