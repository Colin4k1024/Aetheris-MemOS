import React from 'react';
import { Card, Empty } from 'antd';

export interface ChartCardProps {
  title: string;
  loading?: boolean;
  empty?: boolean;
  extra?: React.ReactNode;
  children?: React.ReactNode;
  height?: number;
}

const ChartCard: React.FC<ChartCardProps> = ({
  title,
  loading = false,
  empty = false,
  extra,
  children,
  height = 300,
}) => {
  return (
    <Card title={title} loading={loading} extra={extra}>
      {empty ? (
        <div style={{ height, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <Empty description="暂无数据" />
        </div>
      ) : (
        children
      )}
    </Card>
  );
};

export default ChartCard;
