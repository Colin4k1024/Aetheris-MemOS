import React from 'react';
import { Tag, Space } from 'antd';
import { MEMORY_LAYER_COLORS } from '@/config/appConfig';

export interface MemoryWeights {
  stm: number;
  ltm: number;
  kg: number;
  mm: number;
}

export interface MemoryWeightBadgeProps {
  weights: MemoryWeights;
  /** Show numeric weight value next to each badge */
  showValue?: boolean;
}

const LAYER_LABELS: Record<keyof MemoryWeights, string> = {
  stm: 'STM',
  ltm: 'LTM',
  kg: 'KG',
  mm: 'MM',
};

const LAYER_COLORS: Record<keyof MemoryWeights, string> = {
  stm: MEMORY_LAYER_COLORS.STM,
  ltm: MEMORY_LAYER_COLORS.LTM,
  kg: MEMORY_LAYER_COLORS.KG,
  mm: MEMORY_LAYER_COLORS.MM,
};

const MemoryWeightBadge: React.FC<MemoryWeightBadgeProps> = ({
  weights,
  showValue = true,
}) => {
  return (
    <Space wrap>
      {(Object.keys(LAYER_LABELS) as Array<keyof MemoryWeights>).map((key) => (
        <Tag key={key} color={LAYER_COLORS[key]}>
          {LAYER_LABELS[key]}
          {showValue ? `: ${weights[key].toFixed(2)}` : ''}
        </Tag>
      ))}
    </Space>
  );
};

export default MemoryWeightBadge;
