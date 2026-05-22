import React, { useMemo } from 'react';
import { PageContainer, ProTable } from '@ant-design/pro-components';
import { Row, Col, Tag } from 'antd';
import { Line, Scatter } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getWeightHistory } from '@/services/memory';
import type { ProColumns } from '@ant-design/pro-components';
import { MetricCard, ChartCard } from '@/components/MemorySystem';
import { formatDateTime, formatWeight } from '@/utils/formatters';
import { CHART_HEIGHT } from '@/config/appConfig';

export default function WeightHistoryPage() {
  const { data: rawData, loading } = useRequest(getWeightHistory, {
    formatResult: (r: any) => r,
  });
  const data = rawData as API.WeightHistoryResponse | undefined;

  const weightTrendData = useMemo(() => {
    if (!data?.adjustment_history) return [];
    return data.adjustment_history.flatMap((item) => [
      { time: item.timestamp, value: item.old_weights.stm, type: 'STM (旧)' },
      { time: item.timestamp, value: item.new_weights.stm, type: 'STM (新)' },
      { time: item.timestamp, value: item.old_weights.ltm, type: 'LTM (旧)' },
      { time: item.timestamp, value: item.new_weights.ltm, type: 'LTM (新)' },
      { time: item.timestamp, value: item.old_weights.kg, type: 'KG (旧)' },
      { time: item.timestamp, value: item.new_weights.kg, type: 'KG (新)' },
      { time: item.timestamp, value: item.old_weights.mm, type: 'MM (旧)' },
      { time: item.timestamp, value: item.new_weights.mm, type: 'MM (新)' },
    ]);
  }, [data]);

  const performanceImpactData = useMemo(() => {
    if (!data?.adjustment_history) return [];
    return data.adjustment_history.map((item) => {
      const weightChange = Math.abs(
        item.new_weights.stm - item.old_weights.stm +
        item.new_weights.ltm - item.old_weights.ltm +
        item.new_weights.kg - item.old_weights.kg +
        item.new_weights.mm - item.old_weights.mm,
      );
      return {
        weightChange,
        performanceImpact: item.performance_impact,
        timestamp: item.timestamp,
      };
    });
  }, [data]);

  const columns: ProColumns<API.HistoryItem>[] = [
    { title: '时间', dataIndex: 'timestamp', valueType: 'dateTime', width: 180 },
    { title: '任务ID', dataIndex: 'task_id', width: 120 },
    {
      title: 'STM 权重',
      dataIndex: ['old_weights', 'stm'],
      render: (_, record) => (
        <span>
          {formatWeight(record.old_weights.stm)} → {formatWeight(record.new_weights.stm)}
        </span>
      ),
    },
    {
      title: 'LTM 权重',
      dataIndex: ['old_weights', 'ltm'],
      render: (_, record) => (
        <span>
          {formatWeight(record.old_weights.ltm)} → {formatWeight(record.new_weights.ltm)}
        </span>
      ),
    },
    {
      title: 'KG 权重',
      dataIndex: ['old_weights', 'kg'],
      render: (_, record) => (
        <span>
          {formatWeight(record.old_weights.kg)} → {formatWeight(record.new_weights.kg)}
        </span>
      ),
    },
    {
      title: 'MM 权重',
      dataIndex: ['old_weights', 'mm'],
      render: (_, record) => (
        <span>
          {formatWeight(record.old_weights.mm)} → {formatWeight(record.new_weights.mm)}
        </span>
      ),
    },
    {
      title: '性能影响',
      dataIndex: 'performance_impact',
      render: (val: any) => (
        <Tag color={val > 0 ? 'green' : 'red'}>
          {val > 0 ? '+' : ''}
          {(val * 100).toFixed(2)}%
        </Tag>
      ),
    },
    { title: '调整原因', dataIndex: 'reason', ellipsis: true },
  ];

  return (
    <PageContainer>
      {data?.summary && (
        <Row gutter={16} style={{ marginBottom: 16 }}>
          <Col span={8}>
            <MetricCard
              title="总调整次数"
              value={data.summary.total_adjustments}
              loading={loading}
            />
          </Col>
          <Col span={8}>
            <MetricCard
              title="平均性能影响"
              value={(data.summary.average_performance_impact * 100).toFixed(2)}
              unit="%"
              loading={loading}
            />
          </Col>
          <Col span={8}>
            <MetricCard
              title="最常见调整"
              value={data.summary.most_common_adjustment}
              loading={loading}
            />
          </Col>
        </Row>
      )}

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <ChartCard
            title="权重变化趋势"
            loading={loading}
            empty={weightTrendData.length === 0}
            height={CHART_HEIGHT}
          >
            <Line
              data={weightTrendData}
              xField="time"
              yField="value"
              seriesField="type"
              smooth
              point={{ size: 3 }}
              legend={{ position: 'top' }}
              xAxis={{ type: 'time', label: { formatter: formatDateTime } }}
              yAxis={{ label: { formatter: (t: string) => parseFloat(t).toFixed(2) } }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <ChartCard
            title="权重调整 vs 性能影响"
            loading={loading}
            empty={performanceImpactData.length === 0}
            height={CHART_HEIGHT}
          >
            <Scatter
              data={performanceImpactData}
              xField="weightChange"
              yField="performanceImpact"
              pointStyle={{ size: 5 }}
              regressionLine={{ type: 'linear' }}
              xAxis={{ title: { text: '权重变化量' } }}
              yAxis={{ title: { text: '性能影响' } }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <ProTable<API.HistoryItem>
        headerTitle="调整记录"
        search={false}
        loading={loading}
        dataSource={data?.adjustment_history || []}
        columns={columns}
        rowKey="timestamp"
        pagination={{ pageSize: 10 }}
      />
    </PageContainer>
  );
}
