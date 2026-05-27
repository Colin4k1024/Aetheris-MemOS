import React from 'react';
import { Tag } from 'antd';

export type SystemStatus = 'healthy' | 'warning' | 'error' | string;

const STATUS_COLOR_MAP: Record<string, string> = {
  healthy: 'green',
  active: 'green',
  warning: 'orange',
  error: 'red',
  inactive: 'default',
  testing: 'blue',
  suboptimal: 'orange',
  optimal: 'green',
};

export interface StatusTagProps {
  status: SystemStatus;
  /** Custom label; falls back to the status value if not provided */
  label?: string;
}

const StatusTag: React.FC<StatusTagProps> = ({ status, label }) => {
  const color = STATUS_COLOR_MAP[status] ?? 'default';
  return <Tag color={color}>{label ?? status}</Tag>;
};

export default StatusTag;
