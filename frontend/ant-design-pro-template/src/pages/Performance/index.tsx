import { Column, Line, Radar } from '@ant-design/charts';
import { PageContainer } from '@ant-design/pro-components';
import { useRequest } from '@umijs/max';
import { Col, Descriptions, Row } from 'antd';
import React, { useState } from 'react';
import { ChartCard, MetricCard } from '@/components/MemorySystem';
import { CHART_HEIGHT, POLLING_INTERVALS } from '@/config/appConfig';
import usePolling from '@/hooks/usePolling';
import { getBaselines, getMemoryStatus } from '@/services/memory';
import { formatPercent, formatTime } from '@/utils/formatters';

export default function PerformancePage() {
  const [baselines, setBaselines] = useState<API.BaselinesResponse | null>(
    null,
  );
  const [status, setStatus] = useState<API.MemoryStatusResponse | null>(null);
  const [performanceHistory, setPerformanceHistory] = useState<any[]>([]);

  const { loading: baselinesLoading } = useRequest(getBaselines, {
    formatResult: (r: any) => r,
    onSuccess: (data: any) => setBaselines(data as API.BaselinesResponse),
  });

  const { loading: statusLoading, run: fetchStatus } = useRequest(
    getMemoryStatus,
    {
      manual: true,
      formatResult: (r: any) => r,
      onSuccess: (data: any) => {
        const d = data as API.MemoryStatusResponse;
        setStatus(d);
        if (d?.performance_metrics) {
          const now = Date.now();
          setPerformanceHistory((prev) => {
            const next = [
              ...prev,
              {
                time: now,
                efficiency: d.performance_metrics.efficiency_score,
                coherence: d.performance_metrics.coherence_score,
                responseTime: d.performance_metrics.response_time_ms,
                cpuUsage: d.performance_metrics.cpu_usage_percent,
              },
            ];
            return next.slice(-20);
          });
        }
      },
    },
  );

  usePolling(fetchStatus, { interval: POLLING_INTERVALS.SLOW });

  const timeSeriesData = performanceHistory.flatMap((item) => [
    { time: item.time, value: item.efficiency, type: '效率得分' },
    { time: item.time, value: item.coherence, type: '连贯性得分' },
  ]);

  const contributionData = baselines
    ? [
        {
          type: 'STM',
          metric: '效率',
          value: baselines.performance_baselines.stm.efficiency_gain * 100,
        },
        {
          type: 'LTM',
          metric: '效率',
          value: baselines.performance_baselines.ltm.efficiency_gain * 100,
        },
        {
          type: 'KG',
          metric: '效率',
          value: baselines.performance_baselines.kg.efficiency_gain * 100,
        },
        {
          type: 'MM',
          metric: '效率',
          value: baselines.performance_baselines.mm.efficiency_gain * 100,
        },
        {
          type: 'STM',
          metric: '连贯性',
          value: baselines.performance_baselines.stm.coherence_gain,
        },
        {
          type: 'LTM',
          metric: '连贯性',
          value: baselines.performance_baselines.ltm.coherence_gain,
        },
        {
          type: 'KG',
          metric: '连贯性',
          value: baselines.performance_baselines.kg.coherence_gain,
        },
        {
          type: 'MM',
          metric: '连贯性',
          value: baselines.performance_baselines.mm.coherence_gain,
        },
      ]
    : [];

  return (
    <PageContainer>
      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={6}>
          <MetricCard
            title="效率得分"
            value={(
              (status?.performance_metrics?.efficiency_score || 0) * 100
            ).toFixed(2)}
            unit="%"
            loading={statusLoading}
            variant="success"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="连贯性得分"
            value={status?.performance_metrics?.coherence_score || 0}
            precision={2}
            loading={statusLoading}
            variant="primary"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="响应时间"
            value={status?.performance_metrics?.response_time_ms || 0}
            unit="ms"
            loading={statusLoading}
            variant="info"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="CPU 使用率"
            value={status?.performance_metrics?.cpu_usage_percent || 0}
            unit="%"
            loading={statusLoading}
            variant="warning"
          />
        </Col>
      </Row>

      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={24}>
          <ChartCard
            title="性能指标时间序列"
            loading={statusLoading}
            empty={timeSeriesData.length === 0}
            height={CHART_HEIGHT}
          >
            <Line
              data={timeSeriesData}
              xField="time"
              yField="value"
              seriesField="type"
              smooth
              point={{ size: 4 }}
              legend={{ position: 'top' }}
              xAxis={{ type: 'time', label: { formatter: formatTime } }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={12}>
          <ChartCard
            title="各记忆层贡献度对比"
            loading={baselinesLoading}
            empty={contributionData.length === 0}
            height={CHART_HEIGHT}
          >
            <Column
              data={contributionData}
              xField="type"
              yField="value"
              seriesField="metric"
              isGroup
              columnStyle={{
                radius: [4, 4, 0, 0] as [number, number, number, number],
              }}
              label={{ position: 'top' }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
        <Col span={12}>
          <ChartCard
            title="性能基准对比雷达图"
            loading={baselinesLoading}
            empty={!baselines}
            height={CHART_HEIGHT}
          >
            {baselines && (
              <Radar
                data={[
                  {
                    item: '效率提升',
                    STM:
                      baselines.performance_baselines.stm.efficiency_gain * 100,
                    LTM:
                      baselines.performance_baselines.ltm.efficiency_gain * 100,
                    KG:
                      baselines.performance_baselines.kg.efficiency_gain * 100,
                    MM:
                      baselines.performance_baselines.mm.efficiency_gain * 100,
                  },
                  {
                    item: '连贯性提升',
                    STM: baselines.performance_baselines.stm.coherence_gain,
                    LTM: baselines.performance_baselines.ltm.coherence_gain,
                    KG: baselines.performance_baselines.kg.coherence_gain,
                    MM: baselines.performance_baselines.mm.coherence_gain,
                  },
                ]}
                xField="item"
                yField="value"
                seriesField="type"
                area={{}}
                point={{ size: 4 }}
                legend={{ position: 'top' }}
                height={CHART_HEIGHT}
              />
            )}
          </ChartCard>
        </Col>
      </Row>

      <ChartCard
        title="性能基准数据"
        loading={baselinesLoading}
        empty={!baselines}
      >
        {baselines && (
          <Descriptions column={2} bordered>
            <Descriptions.Item label="STM 效率提升">
              {formatPercent(
                baselines.performance_baselines.stm.efficiency_gain,
              )}
            </Descriptions.Item>
            <Descriptions.Item label="STM 连贯性提升">
              {baselines.performance_baselines.stm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="LTM 效率提升">
              {formatPercent(
                baselines.performance_baselines.ltm.efficiency_gain,
              )}
            </Descriptions.Item>
            <Descriptions.Item label="LTM 连贯性提升">
              {baselines.performance_baselines.ltm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="KG 效率提升">
              {formatPercent(
                baselines.performance_baselines.kg.efficiency_gain,
              )}
            </Descriptions.Item>
            <Descriptions.Item label="KG 连贯性提升">
              {baselines.performance_baselines.kg.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="MM 效率提升">
              {formatPercent(
                baselines.performance_baselines.mm.efficiency_gain,
              )}
            </Descriptions.Item>
            <Descriptions.Item label="MM 连贯性提升">
              {baselines.performance_baselines.mm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
          </Descriptions>
        )}
      </ChartCard>

      <div style={{ marginTop: 24 }}>
        <ChartCard
          title="边际递减系数"
          loading={baselinesLoading}
          empty={!baselines}
        >
          {baselines && (
            <Descriptions column={3} bordered>
              <Descriptions.Item label="STM → LTM">
                {baselines.marginal_decay_factors.stm_to_ltm.toFixed(3)}
              </Descriptions.Item>
              <Descriptions.Item label="LTM → KG">
                {baselines.marginal_decay_factors.ltm_to_kg.toFixed(3)}
              </Descriptions.Item>
              <Descriptions.Item label="KG → MM">
                {baselines.marginal_decay_factors.kg_to_mm.toFixed(3)}
              </Descriptions.Item>
            </Descriptions>
          )}
        </ChartCard>
      </div>
    </PageContainer>
  );
}
