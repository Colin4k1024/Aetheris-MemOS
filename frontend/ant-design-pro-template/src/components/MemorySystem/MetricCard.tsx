import React from 'react';
import { Card, Statistic } from 'antd';
import type { StatisticProps } from 'antd';

export interface MetricCardProps {
  title: string;
  value: string | number;
  unit?: string;
  color?: string;
  icon?: React.ReactNode;
  loading?: boolean;
  precision?: number;
}

const MetricCard: React.FC<MetricCardProps> = ({
  title,
  value,
  unit,
  color,
  icon,
  loading = false,
  precision,
}) => {
  const statisticProps: StatisticProps = {
    title,
    value,
    suffix: unit,
    prefix: icon,
  };
  if (color) {
    statisticProps.valueStyle = { color };
  }
  if (precision !== undefined) {
    statisticProps.precision = precision;
  }

  return (
    <Card loading={loading}>
      <Statistic {...statisticProps} />
    </Card>
  );
};

export default MetricCard;
