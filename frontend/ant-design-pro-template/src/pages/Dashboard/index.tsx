import { PageContainer } from '@ant-design/pro-components';
import { Col, Row } from 'antd';
import { Line, Pie, Column } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getMemoryStatus, healthCheck } from '@/services/memory';
import { useState } from 'react';
import { MetricCard, ChartCard, MemoryWeightBadge } from '@/components/MemorySystem';
import usePolling from '@/hooks/usePolling';
import { formatTime, formatPercent } from '@/utils/formatters';
import { POLLING_INTERVALS, CHART_HEIGHT } from '@/config/appConfig';

export default function DashboardPage() {
  const [status, setStatus] = useState<API.MemoryStatusResponse | null>(null);
  const [health, setHealth] = useState<API.HealthResponse | null>(null);
  const [performanceHistory, setPerformanceHistory] = useState<any[]>([]);

  const { loading: statusLoading, run: fetchStatus } = useRequest(getMemoryStatus, {
    manual: true,
    onSuccess: (data) => {
      setStatus(data);
      if (data) {
        const now = Date.now();
        setPerformanceHistory((prev) => {
          const next = [
            ...prev,
            {
              time: now,
              efficiency: data.performance_metrics.efficiency_score,
              coherence: data.performance_metrics.coherence_score,
            },
          ];
          return next.slice(-20);
        });
      }
    },
  });

  const { loading: healthLoading } = useRequest(healthCheck, {
    onSuccess: (data) => setHealth(data),
  });

  usePolling(fetchStatus, { interval: POLLING_INTERVALS.NORMAL });

  const perfChartData = performanceHistory.flatMap((item) => [
    { time: item.time, value: item.efficiency, type: '效率' },
    { time: item.time, value: item.coherence, type: '连贯性' },
  ]);

  const resourcePieData = status
    ? [
        { type: '内存', value: status.resource_status.memory_usage_percent },
        { type: 'CPU', value: status.resource_status.cpu_usage_percent },
        { type: '存储', value: status.resource_status.storage_usage_percent },
      ]
    : [];

  const memoryUsageData = status
    ? [
        { type: 'STM', value: status.current_config.memory_weights.stm * 100 },
        { type: 'LTM', value: status.current_config.memory_weights.ltm * 100 },
        { type: 'KG', value: status.current_config.memory_weights.kg * 100 },
        { type: 'MM', value: status.current_config.memory_weights.mm * 100 },
      ]
    : [];

  return (
    <PageContainer>
      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={6}>
          <MetricCard
            title="系统状态"
            value={health?.status || 'unknown'}
            color={health?.status === 'healthy' ? '#3f8600' : '#cf1322'}
            loading={healthLoading}
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="平均性能"
            value={((status?.performance_metrics.efficiency_score || 0) * 100).toFixed(2)}
            unit="%"
            loading={statusLoading}
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="资源使用率"
            value={status?.resource_status.memory_usage_percent || 0}
            unit="%"
            loading={statusLoading}
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="响应时间"
            value={status?.performance_metrics.response_time_ms || 0}
            unit="ms"
            loading={statusLoading}
          />
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={12}>
          <ChartCard
            title="性能趋势"
            loading={statusLoading}
            empty={perfChartData.length === 0}
            height={CHART_HEIGHT}
          >
            <Line
              data={perfChartData}
              xField="time"
              yField="value"
              seriesField="type"
              smooth
              point={{ size: 4 }}
              legend={{ position: 'top' }}
              xAxis={{
                type: 'time',
                label: { formatter: formatTime },
              }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
        <Col span={12}>
          <ChartCard
            title="资源使用分布"
            loading={statusLoading}
            empty={resourcePieData.length === 0}
            height={CHART_HEIGHT}
          >
            <Pie
              data={resourcePieData}
              angleField="value"
              colorField="type"
              radius={0.8}
              label={{ type: 'outer', content: '{name}: {value}%' }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={12}>
          <ChartCard title="当前记忆配置" loading={statusLoading} empty={!status}>
            {status && (
              <div>
                <p><strong>主记忆:</strong> {status.current_config.primary_memory}</p>
                <p><strong>次记忆:</strong> {status.current_config.secondary_memory.join(', ') || '无'}</p>
                <p><strong>推理深度:</strong> {status.current_config.reasoning_depth}</p>
                <p><strong>多模态:</strong> {status.current_config.enable_multimodal ? '启用' : '禁用'}</p>
                <p><strong>权重分布:</strong></p>
                <MemoryWeightBadge weights={status.current_config.memory_weights} />
              </div>
            )}
          </ChartCard>
        </Col>
        <Col span={12}>
          <ChartCard
            title="记忆层使用情况"
            loading={statusLoading}
            empty={memoryUsageData.length === 0}
            height={CHART_HEIGHT}
          >
            <Column
              data={memoryUsageData}
              xField="type"
              yField="value"
              columnStyle={{ fill: '#6366f1' }}
              label={{
                position: 'top',
                formatter: (datum: any) => `${datum.value.toFixed(1)}%`,
              }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <Row gutter={16}>
        <Col span={24}>
          <ChartCard title="系统组件状态" loading={healthLoading} empty={!health}>
            {health && (
              <Row gutter={16}>
                {Object.entries(health.components).map(([key, val]) => (
                  <Col span={4} key={key}>
                    <MetricCard title={key} value={val as string} color="#3f8600" />
                  </Col>
                ))}
              </Row>
            )}
          </ChartCard>
        </Col>
      </Row>
    </PageContainer>
  );
}
