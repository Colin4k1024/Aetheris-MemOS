import { Card, Empty } from 'antd';
import { createStyles } from 'antd-style';
import React from 'react';
import { ACCENT_COLORS, CHART_DEFAULTS, RADIUS, SHADOWS } from '@/theme/tokens';

const useStyles = createStyles(({ css, token }) => ({
  card: css`
    border-radius: ${RADIUS.lg}px;
    box-shadow: ${SHADOWS.card};
    border: none;
    .ant-card-head {
      border-bottom: none;
      padding: 20px 24px 0;
      min-height: auto;
    }
    .ant-card-head-title {
      font-size: 15px;
      font-weight: 600;
      color: ${token.colorText};
      padding: 0;
      position: relative;
      padding-left: 12px;
      &::before {
        content: '';
        position: absolute;
        left: 0;
        top: 2px;
        bottom: 2px;
        width: 3px;
        border-radius: 2px;
        background: ${ACCENT_COLORS.primary};
      }
    }
    .ant-card-body {
      padding: 16px 24px 24px;
    }
  `,
  empty: css`
    display: flex;
    align-items: center;
    justify-content: center;
  `,
}));

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
  height = CHART_DEFAULTS.height,
}) => {
  const { styles } = useStyles();

  return (
    <Card title={title} loading={loading} extra={extra} className={styles.card}>
      {empty ? (
        <div className={styles.empty} style={{ height }}>
          <Empty description="暂无数据" />
        </div>
      ) : (
        children
      )}
    </Card>
  );
};

export default ChartCard;
