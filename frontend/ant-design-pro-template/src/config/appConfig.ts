/**
 * Global configuration for Adaptive Memory System
 */

export const DEFAULT_USER_ID = 'user_1';
export const DEFAULT_AGENT_ID = 'agent_1';

export const API_CONFIG = {
  BASE_URL:
    process.env.NODE_ENV === 'development' ? 'http://127.0.0.1:8008' : '',
  TIMEOUT: 30000,
};

export const POLLING_INTERVALS = {
  FAST: 3000,
  NORMAL: 5000,
  SLOW: 10000,
  VERY_SLOW: 30000,
};

export const CHART_HEIGHT = 320;

export const CHART_COLORS = {
  PRIMARY: '#6366f1',
  SUCCESS: '#10b981',
  WARNING: '#f59e0b',
  ERROR: '#ef4444',
  INFO: '#06b6d4',
  TEXT: 'rgba(0, 0, 0, 0.65)',
  BORDER: '#e2e8f0',
  BACKGROUND: '#ffffff',
};

export const MEMORY_LAYER_COLORS = {
  STM: '#6366f1',
  LTM: '#10b981',
  KG: '#8b5cf6',
  MM: '#f59e0b',
};

export default {
  DEFAULT_USER_ID,
  DEFAULT_AGENT_ID,
  API_CONFIG,
  POLLING_INTERVALS,
  CHART_HEIGHT,
  CHART_COLORS,
  MEMORY_LAYER_COLORS,
};
