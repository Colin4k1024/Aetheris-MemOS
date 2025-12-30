import { PageContainer } from '@ant-design/pro-components';
import { Card, Row, Col, Statistic } from 'antd';
import { Line, Pie, Column } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getMemoryStatus, healthCheck } from '@/services/memory';
import { useEffect, useState } from 'react';

export default function DashboardPage() {
  const [status, setStatus] = useState<API.MemoryStatusResponse | null>(null);
  const [health, setHealth] = useState<API.HealthResponse | null>(null);
  const [performanceHistory, setPerformanceHistory] = useState<any[]>([]);

  const { loading: statusLoading, run: fetchStatus } = useRequest(getMemoryStatus, {
    onSuccess: (data) => {
      setStatus(data);
      // 模拟性能历史数据（实际应该从API获取）
      if (data) {
        const now = Date.now();
        setPerformanceHistory([
          { time: now - 3600000, efficiency: data.performance_metrics.efficiency_score * 0.9, coherence: data.performance_metrics.coherence_score * 0.95 },
          { time: now - 1800000, efficiency: data.performance_metrics.efficiency_score * 0.95, coherence: data.performance_metrics.coherence_score * 0.98 },
          { time: now, efficiency: data.performance_metrics.efficiency_score, coherence: data.performance_metrics.coherence_score },
        ]);
      }
    },
  });

  const { loading: healthLoading } = useRequest(healthCheck, {
    onSuccess: (data) => {
      setHealth(data);
    },
  });

  // 自动刷新
  useEffect(() => {
    const interval = setInterval(() => {
      fetchStatus();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // 性能趋势图配置
  const performanceChartConfig = {
    data: performanceHistory,
    xField: 'time',
    yField: 'value',
    seriesField: 'type',
    smooth: true,
    point: {
      size: 4,
      shape: 'circle',
    },
    legend: {
      position: 'top' as const,
    },
    xAxis: {
      type: 'time',
      label: {
        formatter: (text: string) => {
          const date = new Date(parseInt(text));
          return `${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}`;
        },
      },
    },
  };

  // 资源使用饼图配置
  const resourcePieConfig = status ? {
    data: [
      { type: '内存', value: status.resource_status.memory_usage_percent },
      { type: 'CPU', value: status.resource_status.cpu_usage_percent },
      { type: '存储', value: status.resource_status.storage_usage_percent },
    ],
    angleField: 'value',
    colorField: 'type',
    radius: 0.8,
    label: {
      type: 'outer',
      content: '{name}: {value}%',
    },
  } : { data: [] };

  // 记忆层使用情况柱状图配置
  const memoryUsageConfig = status ? {
    data: [
      { type: 'STM', value: status.current_config.memory_weights.stm * 100 },
      { type: 'LTM', value: status.current_config.memory_weights.ltm * 100 },
      { type: 'KG', value: status.current_config.memory_weights.kg * 100 },
      { type: 'MM', value: status.current_config.memory_weights.mm * 100 },
    ],
    xField: 'type',
    yField: 'value',
    columnStyle: {
      fill: '#1890ff',
    },
    label: {
      position: 'top' as const,
      formatter: (datum: any) => `${datum.value.toFixed(1)}%`,
    },
  } : { data: [] };

  return (
    <PageContainer>
      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={6}>
          <Card>
            <Statistic
              title="系统状态"
              value={health?.status || 'unknown'}
              valueStyle={{ color: health?.status === 'healthy' ? '#3f8600' : '#cf1322' }}
            />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic
              title="平均性能"
              value={((status?.performance_metrics.efficiency_score || 0) * 100).toFixed(2)}
              precision={2}
              suffix="%"
            />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic
              title="资源使用率"
              value={status?.resource_status.memory_usage_percent || 0}
              suffix="%"
            />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic
              title="响应时间"
              value={status?.performance_metrics.response_time_ms || 0}
              suffix="ms"
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={12}>
          <Card title="性能趋势" loading={statusLoading}>
            {performanceHistory.length > 0 && (
              <Line
                {...performanceChartConfig}
                data={performanceHistory.flatMap(item => [
                  { time: item.time, value: item.efficiency, type: '效率' },
                  { time: item.time, value: item.coherence, type: '连贯性' },
                ])}
              />
            )}
          </Card>
        </Col>
        <Col span={12}>
          <Card title="资源使用分布" loading={statusLoading}>
            {status && <Pie {...resourcePieConfig} />}
          </Card>
        </Col>
      </Row>

      <Row gutter={16}>
        <Col span={12}>
          <Card title="当前记忆配置" loading={statusLoading}>
            {status && (
              <div>
                <p>主记忆: {status.current_config.primary_memory}</p>
                <p>次记忆: {status.current_config.secondary_memory.join(', ') || '无'}</p>
                <p>推理深度: {status.current_config.reasoning_depth}</p>
                <p>多模态: {status.current_config.enable_multimodal ? '启用' : '禁用'}</p>
              </div>
            )}
          </Card>
        </Col>
        <Col span={12}>
          <Card title="记忆层使用情况" loading={statusLoading}>
            {status && <Column {...memoryUsageConfig} />}
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginTop: 16 }}>
        <Col span={24}>
          <Card title="系统组件状态" loading={healthLoading}>
            {health && (
              <Row gutter={16}>
                <Col span={4}>
                  <Statistic title="调度器" value={health.components.scheduler} valueStyle={{ color: '#3f8600' }} />
                </Col>
                <Col span={4}>
                  <Statistic title="分析器" value={health.components.analyzer} valueStyle={{ color: '#3f8600' }} />
                </Col>
                <Col span={4}>
                  <Statistic title="预测器" value={health.components.predictor} valueStyle={{ color: '#3f8600' }} />
                </Col>
                <Col span={4}>
                  <Statistic title="监控器" value={health.components.monitor} valueStyle={{ color: '#3f8600' }} />
                </Col>
                <Col span={4}>
                  <Statistic title="权重调整器" value={health.components.weight_adjuster} valueStyle={{ color: '#3f8600' }} />
                </Col>
              </Row>
            )}
          </Card>
        </Col>
      </Row>
    </PageContainer>
  );
}
