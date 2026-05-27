import type { StatisticProps } from 'antd';
import { Card, Statistic } from 'antd';
import { createStyles } from 'antd-style';
import React from 'react';
import { ACCENT_COLORS, GRADIENTS, RADIUS, SHADOWS } from '@/theme/tokens';

const useStyles = createStyles(({ css }) => ({
  card: css`
    border-radius: ${RADIUS.lg}px;
    box-shadow: ${SHADOWS.card};
    transition: all 0.3s ease;
    overflow: hidden;
    border: none;
    &:hover {
      box-shadow: ${SHADOWS.cardHover};
      transform: translateY(-2px);
    }
  `,
  primary: css`
    background: ${GRADIENTS.cardPrimary};
    border-left: 3px solid ${ACCENT_COLORS.primary};
  `,
  success: css`
    background: ${GRADIENTS.cardSuccess};
    border-left: 3px solid ${ACCENT_COLORS.success};
  `,
  warning: css`
    background: ${GRADIENTS.cardWarning};
    border-left: 3px solid ${ACCENT_COLORS.warning};
  `,
  info: css`
    background: ${GRADIENTS.cardInfo};
    border-left: 3px solid ${ACCENT_COLORS.info};
  `,
  default: css`
    background: #ffffff;
  `,
}));

export type MetricCardVariant =
  | 'primary'
  | 'success'
  | 'warning'
  | 'info'
  | 'default';

export interface MetricCardProps {
  title: string;
  value: string | number;
  unit?: string;
  color?: string;
  icon?: React.ReactNode;
  loading?: boolean;
  precision?: number;
  variant?: MetricCardVariant;
}

const MetricCard: React.FC<MetricCardProps> = ({
  title,
  value,
  unit,
  color,
  icon,
  loading = false,
  precision,
  variant = 'default',
}) => {
  const { styles, cx } = useStyles();

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
    <Card loading={loading} className={cx(styles.card, styles[variant])}>
      <Statistic {...statisticProps} />
    </Card>
  );
};

export default MetricCard;
