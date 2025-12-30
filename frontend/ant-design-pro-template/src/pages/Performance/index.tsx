import { PageContainer } from '@ant-design/pro-components';
import { Card, Descriptions, Statistic, Row, Col } from 'antd';
import { Line, Radar, Column } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getBaselines, getMemoryStatus } from '@/services/memory';
import { useEffect, useState } from 'react';

export default function PerformancePage() {
  const [baselines, setBaselines] = useState<API.BaselinesResponse | null>(null);
  const [status, setStatus] = useState<API.MemoryStatusResponse | null>(null);
  const [performanceHistory, setPerformanceHistory] = useState<any[]>([]);

  const { loading: baselinesLoading } = useRequest(getBaselines, {
    onSuccess: (data) => {
      setBaselines(data);
    },
  });

  const { loading: statusLoading, run: fetchStatus } = useRequest(getMemoryStatus, {
    onSuccess: (data) => {
      setStatus(data);
      // 模拟性能历史数据
      if (data) {
        const now = Date.now();
        const history = [];
        for (let i = 5; i >= 0; i--) {
          const time = now - i * 600000; // 每10分钟一个数据点
          history.push({
            time,
            efficiency: data.performance_metrics.efficiency_score * (0.9 + Math.random() * 0.2),
            coherence: data.performance_metrics.coherence_score * (0.9 + Math.random() * 0.2),
            responseTime: data.performance_metrics.response_time_ms * (0.8 + Math.random() * 0.4),
            cpuUsage: data.performance_metrics.cpu_usage_percent * (0.8 + Math.random() * 0.4),
          });
        }
        setPerformanceHistory(history);
      }
    },
  });

  useEffect(() => {
    const interval = setInterval(() => {
      fetchStatus();
    }, 10000); // 每10秒刷新
    return () => clearInterval(interval);
  }, []);

  // 性能指标时间序列图配置
  const timeSeriesConfig = {
    data: performanceHistory.flatMap(item => [
      { time: item.time, value: item.efficiency, type: '效率得分' },
      { time: item.time, value: item.coherence, type: '连贯性得分' },
    ]),
    xField: 'time',
    yField: 'value',
    seriesField: 'type',
    smooth: true,
    point: { size: 4 },
    legend: { position: 'top' as const },
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

  // 各记忆层贡献度对比图
  const contributionConfig = baselines ? {
    data: [
      { type: 'STM', efficiency: baselines.performance_baselines.stm.efficiency_gain * 100, coherence: baselines.performance_baselines.stm.coherence_gain },
      { type: 'LTM', efficiency: baselines.performance_baselines.ltm.efficiency_gain * 100, coherence: baselines.performance_baselines.ltm.coherence_gain },
      { type: 'KG', efficiency: baselines.performance_baselines.kg.efficiency_gain * 100, coherence: baselines.performance_baselines.kg.coherence_gain },
      { type: 'MM', efficiency: baselines.performance_baselines.mm.efficiency_gain * 100, coherence: baselines.performance_baselines.mm.coherence_gain },
    ],
    xField: 'type',
    yField: 'value',
    seriesField: 'metric',
    isGroup: true,
    columnStyle: {
      radius: [4, 4, 0, 0],
    },
    label: {
      position: 'top' as const,
    },
  } : { data: [] };

  // 性能基准对比雷达图
  const radarConfig = baselines ? {
    data: [
      {
        item: '效率提升',
        STM: baselines.performance_baselines.stm.efficiency_gain * 100,
        LTM: baselines.performance_baselines.ltm.efficiency_gain * 100,
        KG: baselines.performance_baselines.kg.efficiency_gain * 100,
        MM: baselines.performance_baselines.mm.efficiency_gain * 100,
      },
      {
        item: '连贯性提升',
        STM: baselines.performance_baselines.stm.coherence_gain,
        LTM: baselines.performance_baselines.ltm.coherence_gain,
        KG: baselines.performance_baselines.kg.coherence_gain,
        MM: baselines.performance_baselines.mm.coherence_gain,
      },
    ],
    xField: 'item',
    yField: 'value',
    seriesField: 'type',
    area: {},
    point: { size: 4 },
    legend: { position: 'top' as const },
  } : { data: [] };

  return (
    <PageContainer>
      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={6}>
          <Card>
            <Statistic
              title="效率得分"
              value={((status?.performance_metrics.efficiency_score || 0) * 100).toFixed(2)}
              precision={2}
              suffix="%"
              valueStyle={{ color: '#3f8600' }}
            />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic
              title="连贯性得分"
              value={status?.performance_metrics.coherence_score || 0}
              precision={2}
              valueStyle={{ color: '#1890ff' }}
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
        <Col span={6}>
          <Card>
            <Statistic
              title="CPU 使用率"
              value={status?.performance_metrics.cpu_usage_percent || 0}
              suffix="%"
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <Card title="性能指标时间序列" loading={statusLoading}>
            {performanceHistory.length > 0 && <Line {...timeSeriesConfig} />}
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={12}>
          <Card title="各记忆层贡献度对比" loading={baselinesLoading}>
            {baselines && (
              <Column
                {...contributionConfig}
                data={[
                  { type: 'STM', metric: '效率', value: baselines.performance_baselines.stm.efficiency_gain * 100 },
                  { type: 'LTM', metric: '效率', value: baselines.performance_baselines.ltm.efficiency_gain * 100 },
                  { type: 'KG', metric: '效率', value: baselines.performance_baselines.kg.efficiency_gain * 100 },
                  { type: 'MM', metric: '效率', value: baselines.performance_baselines.mm.efficiency_gain * 100 },
                  { type: 'STM', metric: '连贯性', value: baselines.performance_baselines.stm.coherence_gain },
                  { type: 'LTM', metric: '连贯性', value: baselines.performance_baselines.ltm.coherence_gain },
                  { type: 'KG', metric: '连贯性', value: baselines.performance_baselines.kg.coherence_gain },
                  { type: 'MM', metric: '连贯性', value: baselines.performance_baselines.mm.coherence_gain },
                ]}
              />
            )}
          </Card>
        </Col>
        <Col span={12}>
          <Card title="性能基准对比雷达图" loading={baselinesLoading}>
            {baselines && (
              <Radar
                data={[
                  { item: '效率提升', STM: baselines.performance_baselines.stm.efficiency_gain * 100, LTM: baselines.performance_baselines.ltm.efficiency_gain * 100, KG: baselines.performance_baselines.kg.efficiency_gain * 100, MM: baselines.performance_baselines.mm.efficiency_gain * 100 },
                  { item: '连贯性提升', STM: baselines.performance_baselines.stm.coherence_gain, LTM: baselines.performance_baselines.ltm.coherence_gain, KG: baselines.performance_baselines.kg.coherence_gain, MM: baselines.performance_baselines.mm.coherence_gain },
                ]}
                xField="item"
                yField="value"
                seriesField="type"
                area={{}}
                point={{ size: 4 }}
                legend={{ position: 'top' }}
              />
            )}
          </Card>
        </Col>
      </Row>

      <Card title="性能基准数据" loading={baselinesLoading}>
        {baselines && (
          <Descriptions column={2} bordered>
            <Descriptions.Item label="STM 效率提升">
              {(baselines.performance_baselines.stm.efficiency_gain * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="STM 连贯性提升">
              {baselines.performance_baselines.stm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="LTM 效率提升">
              {(baselines.performance_baselines.ltm.efficiency_gain * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="LTM 连贯性提升">
              {baselines.performance_baselines.ltm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="KG 效率提升">
              {(baselines.performance_baselines.kg.efficiency_gain * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="KG 连贯性提升">
              {baselines.performance_baselines.kg.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="MM 效率提升">
              {(baselines.performance_baselines.mm.efficiency_gain * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="MM 连贯性提升">
              {baselines.performance_baselines.mm.coherence_gain.toFixed(2)}
            </Descriptions.Item>
          </Descriptions>
        )}
      </Card>

      <Card title="边际递减系数" loading={baselinesLoading} style={{ marginTop: 16 }}>
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
      </Card>
    </PageContainer>
  );
}
