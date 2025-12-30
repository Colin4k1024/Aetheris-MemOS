import { PageContainer, ProTable } from '@ant-design/pro-components';
import { Card, Tag, Descriptions, Row, Col } from 'antd';
import { Line, Scatter } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getWeightHistory } from '@/services/memory';
import type { ProColumns } from '@ant-design/pro-components';
import { useState, useMemo } from 'react';

export default function WeightHistoryPage() {
  const { data, loading } = useRequest(getWeightHistory);

  // 权重变化趋势数据
  const weightTrendData = useMemo(() => {
    if (!data?.adjustment_history) return [];
    return data.adjustment_history.flatMap((item, index) => [
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

  // 性能影响散点图数据
  const performanceImpactData = useMemo(() => {
    if (!data?.adjustment_history) return [];
    return data.adjustment_history.map((item) => {
      const weightChange = Math.abs(
        item.new_weights.stm - item.old_weights.stm +
        item.new_weights.ltm - item.old_weights.ltm +
        item.new_weights.kg - item.old_weights.kg +
        item.new_weights.mm - item.old_weights.mm
      );
      return {
        weightChange,
        performanceImpact: item.performance_impact,
        timestamp: item.timestamp,
      };
    });
  }, [data]);

  const columns: ProColumns<API.HistoryItem>[] = [
    {
      title: '时间',
      dataIndex: 'timestamp',
      valueType: 'dateTime',
      width: 180,
    },
    {
      title: '任务ID',
      dataIndex: 'task_id',
      width: 120,
    },
    {
      title: 'STM 权重',
      dataIndex: ['old_weights', 'stm'],
      render: (_, record) => (
        <span>
          {record.old_weights.stm.toFixed(2)} → {record.new_weights.stm.toFixed(2)}
        </span>
      ),
    },
    {
      title: 'LTM 权重',
      dataIndex: ['old_weights', 'ltm'],
      render: (_, record) => (
        <span>
          {record.old_weights.ltm.toFixed(2)} → {record.new_weights.ltm.toFixed(2)}
        </span>
      ),
    },
    {
      title: 'KG 权重',
      dataIndex: ['old_weights', 'kg'],
      render: (_, record) => (
        <span>
          {record.old_weights.kg.toFixed(2)} → {record.new_weights.kg.toFixed(2)}
        </span>
      ),
    },
    {
      title: 'MM 权重',
      dataIndex: ['old_weights', 'mm'],
      render: (_, record) => (
        <span>
          {record.old_weights.mm.toFixed(2)} → {record.new_weights.mm.toFixed(2)}
        </span>
      ),
    },
    {
      title: '性能影响',
      dataIndex: 'performance_impact',
      render: (val) => (
        <Tag color={val > 0 ? 'green' : 'red'}>
          {val > 0 ? '+' : ''}
          {(val * 100).toFixed(2)}%
        </Tag>
      ),
    },
    {
      title: '调整原因',
      dataIndex: 'reason',
      ellipsis: true,
    },
  ];

  // 权重变化趋势图配置
  const weightTrendConfig = {
    data: weightTrendData,
    xField: 'time',
    yField: 'value',
    seriesField: 'type',
    smooth: true,
    point: { size: 3 },
    legend: { position: 'top' as const },
    xAxis: {
      type: 'time',
      label: {
        formatter: (text: string) => {
          try {
            const date = new Date(text);
            return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}`;
          } catch {
            return text;
          }
        },
      },
    },
    yAxis: {
      label: {
        formatter: (text: string) => parseFloat(text).toFixed(2),
      },
    },
  };

  // 性能影响散点图配置
  const scatterConfig = {
    data: performanceImpactData,
    xField: 'weightChange',
    yField: 'performanceImpact',
    point: {
      size: 5,
      shape: 'circle',
    },
    regressionLine: {
      type: 'linear' as const,
    },
    xAxis: {
      title: { text: '权重变化量' },
    },
    yAxis: {
      title: { text: '性能影响' },
    },
  };

  return (
    <PageContainer>
      <Card title="权重调整历史" style={{ marginBottom: 16 }}>
        {data?.summary && (
          <Descriptions column={3} bordered style={{ marginBottom: 16 }}>
            <Descriptions.Item label="总调整次数">
              {data.summary.total_adjustments}
            </Descriptions.Item>
            <Descriptions.Item label="平均性能影响">
              {(data.summary.average_performance_impact * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="最常见调整">
              {data.summary.most_common_adjustment}
            </Descriptions.Item>
          </Descriptions>
        )}
      </Card>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <Card title="权重变化趋势" loading={loading}>
            {weightTrendData.length > 0 && <Line {...weightTrendConfig} />}
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <Card title="权重调整 vs 性能影响" loading={loading}>
            {performanceImpactData.length > 0 && <Scatter {...scatterConfig} />}
          </Card>
        </Col>
      </Row>

      <ProTable<API.HistoryItem>
        headerTitle="调整记录"
        search={false}
        loading={loading}
        dataSource={data?.adjustment_history || []}
        columns={columns}
        rowKey="timestamp"
        pagination={{
          pageSize: 10,
        }}
      />
    </PageContainer>
  );
}
