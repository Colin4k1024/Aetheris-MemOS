import { PageContainer, ProForm, ProFormSelect, ProFormSlider } from '@ant-design/pro-components';
import { Card, message, Descriptions, Tag, Space, Row, Col, Table } from 'antd';
import { Column, Line } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { selectMemoryConfig, predictPerformance, getMemoryStatus } from '@/services/memory';
import { useState, useMemo } from 'react';

export default function MemoryConfigPage() {
  const [configResult, setConfigResult] = useState<API.SelectMemoryResponse | null>(null);
  const [predictionResult, setPredictionResult] = useState<API.PredictPerformanceResponse | null>(null);
  const [currentStatus, setCurrentStatus] = useState<API.MemoryStatusResponse | null>(null);
  const [configHistory, setConfigHistory] = useState<API.SelectMemoryResponse[]>([]);

  const { loading: statusLoading } = useRequest(getMemoryStatus, {
    onSuccess: (data) => {
      setCurrentStatus(data);
    },
  });

  const { loading: configLoading, run: selectConfig } = useRequest(selectMemoryConfig, {
    manual: true,
    onSuccess: (data) => {
      setConfigResult(data);
      setConfigHistory(prev => [...prev, data].slice(-5)); // 保留最近5个配置
      message.success('配置选择完成');
    },
    onError: () => {
      message.error('配置选择失败');
    },
  });

  const { loading: predictLoading, run: predict } = useRequest(predictPerformance, {
    manual: true,
    onSuccess: (data) => {
      setPredictionResult(data);
      message.success('性能预测完成');
    },
  });

  const handleSelectConfig = async (values: any) => {
    const request: API.SelectMemoryRequest = {
      task_context: {
        task_id: `task_${Date.now()}`,
        task_type: values.task_type || 'query',
        complexity: values.complexity || 0.5,
        modality_requirements: values.modality_requirements || [],
        temporal_scope: values.temporal_scope || 'medium',
        reasoning_depth: values.reasoning_depth || 'medium',
        context_dependency: values.context_dependency || 0.5,
        user_id: 'user_1',
        agent_id: 'agent_1',
      },
      resource_constraints: {
        max_memory_usage_mb: values.max_memory_usage_mb || 1024,
        max_cpu_usage_percent: values.max_cpu_usage_percent || 80,
        max_response_time_ms: values.max_response_time_ms || 2000,
        storage_quota_percent: values.storage_quota_percent || 90,
      },
      preferences: {
        prioritize_efficiency: values.prioritize_efficiency || true,
        prioritize_coherence: values.prioritize_coherence || false,
        enable_multimodal: values.enable_multimodal || false,
        enable_reasoning: values.enable_reasoning || true,
      },
    };
    await selectConfig(request);
  };

  const handlePredict = async () => {
    if (!configResult || !configResult.memory_config) {
      message.warning('请先选择记忆配置');
      return;
    }
    const request: API.PredictPerformanceRequest = {
      task_profile: {
        complexity: 0.75,
        modality_count: 2,
        temporal_scope: 'medium',
        reasoning_depth: 0.8,
        context_dependency: 0.6,
      },
      memory_config: configResult.memory_config,
    };
    await predict(request);
  };

  // 配置对比数据
  const comparisonData = useMemo(() => {
    if (!configResult || !currentStatus) return [];
    return [
      {
        type: 'STM',
        current: currentStatus.current_config.memory_weights.stm,
        selected: configResult.memory_config.memory_weights.stm,
      },
      {
        type: 'LTM',
        current: currentStatus.current_config.memory_weights.ltm,
        selected: configResult.memory_config.memory_weights.ltm,
      },
      {
        type: 'KG',
        current: currentStatus.current_config.memory_weights.kg,
        selected: configResult.memory_config.memory_weights.kg,
      },
      {
        type: 'MM',
        current: currentStatus.current_config.memory_weights.mm,
        selected: configResult.memory_config.memory_weights.mm,
      },
    ];
  }, [configResult, currentStatus]);

  // 配置对比图
  const comparisonConfig = {
    data: comparisonData.flatMap(item => [
      { type: item.type, value: item.current, config: '当前配置' },
      { type: item.type, value: item.selected, config: '选中配置' },
    ]),
    xField: 'type',
    yField: 'value',
    seriesField: 'config',
    isGroup: true,
    columnStyle: {
      radius: [4, 4, 0, 0],
    },
    label: {
      position: 'top' as const,
      formatter: (datum: any) => datum.value.toFixed(2),
    },
  };

  // 配置历史趋势
  const historyConfig = {
    data: configHistory
      .filter((config) => config && config.memory_config && config.memory_config.memory_weights)
      .flatMap((config, index) => [
        { index, value: config.memory_config.memory_weights.stm, type: 'STM' },
        { index, value: config.memory_config.memory_weights.ltm, type: 'LTM' },
        { index, value: config.memory_config.memory_weights.kg, type: 'KG' },
        { index, value: config.memory_config.memory_weights.mm, type: 'MM' },
      ]),
    xField: 'index',
    yField: 'value',
    seriesField: 'type',
    smooth: true,
    point: { size: 3 },
    legend: { position: 'top' as const },
  };

  return (
    <PageContainer>
      <Card title="自适应记忆配置" style={{ marginBottom: 16 }}>
        <ProForm
          onFinish={handleSelectConfig}
          submitter={{
            searchConfig: {
              submitText: '选择配置',
            },
          }}
        >
          <ProFormSelect
            name="task_type"
            label="任务类型"
            options={[
              { label: '对话', value: 'conversation' },
              { label: '任务', value: 'task' },
              { label: '查询', value: 'query' },
            ]}
          />
          <ProFormSlider
            name="complexity"
            label="任务复杂度"
            min={0}
            max={1}
            step={0.01}
            marks={{ 0: '简单', 0.5: '中等', 1: '复杂' }}
          />
          <ProFormSelect
            name="modality_requirements"
            label="模态需求"
            mode="multiple"
            options={[
              { label: '文本', value: 'text' },
              { label: '图像', value: 'image' },
              { label: '音频', value: 'audio' },
              { label: '视频', value: 'video' },
            ]}
          />
          <ProFormSelect
            name="reasoning_depth"
            label="推理深度"
            options={[
              { label: '浅', value: 'shallow' },
              { label: '中', value: 'medium' },
              { label: '深', value: 'deep' },
            ]}
          />
          <ProFormSlider
            name="max_memory_usage_mb"
            label="最大内存 (MB)"
            min={256}
            max={2048}
            step={256}
          />
          <ProFormSlider
            name="max_cpu_usage_percent"
            label="最大 CPU 使用率 (%)"
            min={20}
            max={100}
            step={10}
          />
        </ProForm>
      </Card>

      {configResult && configResult.memory_config && currentStatus && currentStatus.current_config && (
        <Row gutter={16} style={{ marginBottom: 16 }}>
          <Col span={24}>
            <Card title="配置对比" loading={configLoading}>
              <Column {...comparisonConfig} />
            </Card>
          </Col>
        </Row>
      )}

      {configHistory.length > 0 && (
        <Row gutter={16} style={{ marginBottom: 16 }}>
          <Col span={24}>
            <Card title="配置历史趋势">
              <Line {...historyConfig} />
            </Card>
          </Col>
        </Row>
      )}

      {configResult && configResult.memory_config && configResult.performance_prediction && (
        <Card
          title="记忆配置结果"
          loading={configLoading}
          extra={
            <a onClick={handlePredict} style={{ cursor: 'pointer' }}>
              预测性能
            </a>
          }
        >
          <Descriptions column={2} bordered>
            <Descriptions.Item label="主记忆">
              <Tag>{configResult.memory_config.primary_memory}</Tag>
            </Descriptions.Item>
            <Descriptions.Item label="次记忆">
              <Space>
                {configResult.memory_config.secondary_memory.map((m) => (
                  <Tag key={m}>{m}</Tag>
                ))}
              </Space>
            </Descriptions.Item>
            <Descriptions.Item label="STM 权重">
              {configResult.memory_config.memory_weights.stm.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="LTM 权重">
              {configResult.memory_config.memory_weights.ltm.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="KG 权重">
              {configResult.memory_config.memory_weights.kg.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="MM 权重">
              {configResult.memory_config.memory_weights.mm.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="效率提升">
              {(configResult.performance_prediction.efficiency_gain * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="连贯性提升">
              {configResult.performance_prediction.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="资源成本">
              {configResult.performance_prediction.resource_cost.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="成本效益比">
              {configResult.performance_prediction.cost_benefit_ratio?.toFixed(2) || 'N/A'}
            </Descriptions.Item>
          </Descriptions>
        </Card>
      )}

      {predictionResult && (
        <Card title="性能预测结果" loading={predictLoading} style={{ marginTop: 16 }}>
          <Row gutter={16}>
            <Col span={12}>
              <Descriptions column={1} bordered>
                <Descriptions.Item label="协同因子">
                  {predictionResult.synergy_factor.toFixed(2)}
                </Descriptions.Item>
                <Descriptions.Item label="衰减因子">
                  {predictionResult.decay_factor.toFixed(2)}
                </Descriptions.Item>
              </Descriptions>
            </Col>
            <Col span={12}>
              <Card title="各记忆层贡献度" size="small">
                <Column
                  data={[
                    { type: 'STM', value: predictionResult.performance_breakdown.stm_contribution * 100 },
                    { type: 'LTM', value: predictionResult.performance_breakdown.ltm_contribution * 100 },
                    { type: 'KG', value: predictionResult.performance_breakdown.kg_contribution * 100 },
                    { type: 'MM', value: predictionResult.performance_breakdown.mm_contribution * 100 },
                  ]}
                  xField="type"
                  yField="value"
                  label={{
                    position: 'top' as const,
                    formatter: (datum: any) => `${datum.value.toFixed(2)}%`,
                  }}
                />
              </Card>
            </Col>
          </Row>
        </Card>
      )}
    </PageContainer>
  );
}
