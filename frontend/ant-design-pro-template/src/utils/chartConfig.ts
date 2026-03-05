/**
 * Common chart configurations
 */

import { MEMORY_LAYER_COLORS, CHART_COLORS } from '@/config/appConfig';

// ==================== Base Chart Config ====================

/**
 * Common axis configuration
 */
export const commonAxisConfig = {
  label: {
    formatter: (text: string) => {
      const date = new Date(parseInt(text));
      return `${date.getHours().toString().padStart(2, '0')}:${date
        .getMinutes()
        .toString()
        .padStart(2, '0')}`;
    },
  },
};

/**
 * Common legend configuration
 */
export const commonLegendConfig = {
  position: 'top' as const,
  itemName: {
    formatter: (text: string) => text,
  },
};

/**
 * Common tooltip configuration
 */
export const commonTooltipConfig = {
  showMarkers: true,
  shared: true,
};

// ==================== Line Chart Config ====================

/**
 * Default line chart configuration
 */
export const defaultLineConfig = {
  xField: 'time',
  yField: 'value',
  color: CHART_COLORS.PRIMARY,
  smooth: true,
  animation: {
    appear: {
      animation: 'path-in',
      duration: 1000,
    },
  },
  tooltip: commonTooltipConfig,
  legend: commonLegendConfig,
};

/**
 * Memory weight trend chart configuration
 */
export const weightTrendConfig = {
  xField: 'time',
  yField: 'value',
  colorField: 'type',
  smooth: true,
  animation: {
    appear: {
      animation: 'path-in',
      duration: 1000,
    },
  },
  tooltip: commonTooltipConfig,
  legend: commonLegendConfig,
  style: {
    lineWidth: 2,
  },
};

// ==================== Area Chart Config ====================

/**
 * Default area chart configuration
 */
export const defaultAreaConfig = {
  xField: 'time',
  yField: 'value',
  color: CHART_COLORS.PRIMARY,
  areaStyle: {
    fill: `l(270) 0:#ffffff 1:${CHART_COLORS.PRIMARY}30`,
  },
  animation: {
    appear: {
      animation: 'fade-in',
      duration: 1000,
    },
  },
};

// ==================== Column Chart Config ====================

/**
 * Default column chart configuration
 */
export const defaultColumnConfig = {
  xField: 'type',
  yField: 'value',
  color: CHART_COLORS.PRIMARY,
  label: {
    position: 'top' as const,
  },
  animation: {
    appear: {
      animation: 'scale-in',
      duration: 500,
    },
  },
};

// ==================== Pie Chart Config ====================

/**
 * Default pie chart configuration
 */
export const defaultPieConfig = {
  angleField: 'value',
  colorField: 'type',
  radius: 0.8,
  innerRadius: 0.6,
  label: {
    type: 'inner' as const,
    offset: '-50%',
    content: '{percentage}',
    style: {
      textAlign: 'center' as const,
      fontSize: 14,
    },
  },
  legend: {
    position: 'right' as const,
  },
  statistic: {
    title: {
      content: 'Total',
      style: {
        fontSize: '14px',
      },
    },
    content: {
      style: {
        fontSize: '24px',
      },
    },
  },
};

// ==================== Radar Chart Config ====================

/**
 * Task characteristics radar chart configuration
 */
export const radarConfig = {
  xField: 'feature',
  yField: 'value',
  seriesField: 'category',
  color: [CHART_COLORS.PRIMARY, CHART_COLORS.SUCCESS],
  area: {},
  point: {
    size: 3,
  },
  legend: commonLegendConfig,
  tooltip: commonTooltipConfig,
};

// ==================== Scatter Chart Config ====================

/**
 * Performance scatter chart configuration
 */
export const scatterConfig = {
  xField: 'x',
  yField: 'y',
  colorField: 'category',
  sizeField: 'size',
  point: {
    shape: 'circle',
    style: {
      fillOpacity: 0.7,
    },
  },
  legend: commonLegendConfig,
  tooltip: commonTooltipConfig,
};

// ==================== Gauge Chart Config ====================

/**
 * Resource usage gauge chart configuration
 */
export const gaugeConfig = {
  type: 'gauge',
  min: 0,
  max: 100,
  arc: {
    style: {
      lineWidth: 20,
    },
  },
  indicator: {
    pin: false,
    style: {
      fill: CHART_COLORS.PRIMARY,
    },
  },
  statistic: {
    content: {
      formatter: (value: number) => `${value.toFixed(0)}%`,
      style: {
        fontSize: '24px',
      },
    },
  },
};

// ==================== Helper Functions ====================

/**
 * Generate time series data for charts
 */
export const generateTimeSeriesData = (
  baseTime: number,
  points: number,
  interval: number,
  valueGenerator: (i: number) => number,
): { time: number; value: number }[] => {
  return Array.from({ length: points }, (_, i) => ({
    time: baseTime - (points - 1 - i) * interval,
    value: valueGenerator(i),
  }));
};

/**
 * Convert data to chart format
 */
export const toChartSeries = <T extends Record<string, any>>(
  data: T[],
  valueField: keyof T,
  timeField: keyof T,
  categoryField?: keyof T,
): { time: number; value: number; type?: string }[] => {
  return data.map((item) => ({
    time: Number(item[timeField]),
    value: Number(item[valueField]),
    ...(categoryField ? { type: String(item[categoryField]) } : {}),
  }));
};

export default {
  commonAxisConfig,
  commonLegendConfig,
  commonTooltipConfig,
  defaultLineConfig,
  weightTrendConfig,
  defaultAreaConfig,
  defaultColumnConfig,
  defaultPieConfig,
  radarConfig,
  scatterConfig,
  gaugeConfig,
  generateTimeSeriesData,
  toChartSeries,
};
