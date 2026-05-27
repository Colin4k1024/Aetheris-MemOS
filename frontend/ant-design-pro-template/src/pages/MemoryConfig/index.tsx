import { Column, Line } from '@ant-design/charts';
import {
  PageContainer,
  ProForm,
  ProFormSelect,
  ProFormSlider,
} from '@ant-design/pro-components';
import { useRequest } from '@umijs/max';
import { Col, Descriptions, message, Row, Space, Tag } from 'antd';
import React, { useMemo, useState } from 'react';
import { ChartCard, MemoryWeightBadge } from '@/components/MemorySystem';
import {
  CHART_HEIGHT,
  DEFAULT_AGENT_ID,
  DEFAULT_USER_ID,
} from '@/config/appConfig';
import {
  getMemoryStatus,
  predictPerformance,
  selectMemoryConfig,
} from '@/services/memory';

// ── Sub-components ────────────────────────────────────────────────────────────

interface ConfigInputFormProps {
  onSubmit: (values: any) => Promise<void>;
}

const ConfigInputForm: React.FC<ConfigInputFormProps> = ({ onSubmit }) => (
  <ChartCard title="自适应记忆配置" extra={null}>
    <ProForm
      onFinish={onSubmit}
      submitter={{ searchConfig: { submitText: '选择配置' } }}
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
  </ChartCard>
);

interface ConfigComparisonProps {
  comparisonData: any[];
  historyData: any[];
  configResult: API.SelectMemoryResponse;
  loading: boolean;
  onPredict: () => void;
}

const ConfigComparison: React.FC<ConfigComparisonProps> = ({
  comparisonData,
  historyData,
  configResult,
  loading,
  onPredict,
}) => (
  <>
    {comparisonData.length > 0 && (
      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={24}>
          <ChartCard title="配置对比" loading={loading} height={CHART_HEIGHT}>
            <Column
              data={comparisonData}
              xField="type"
              yField="value"
              seriesField="config"
              isGroup
              columnStyle={{ radius: [4, 4, 0, 0] as any }}
              label={{
                position: 'top',
                formatter: (d: any) => d.value.toFixed(2),
              }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>
    )}

    {historyData.length > 0 && (
      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={24}>
          <ChartCard title="配置历史趋势" height={CHART_HEIGHT}>
            <Line
              data={historyData}
              xField="index"
              yField="value"
              seriesField="type"
              smooth
              point={{ size: 3 }}
              legend={{ position: 'top' }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>
    )}

    {configResult.memory_config && configResult.performance_prediction && (
      <ChartCard
        title="记忆配置结果"
        loading={loading}
        extra={
          <a onClick={onPredict} style={{ cursor: 'pointer' }}>
            预测性能
          </a>
        }
      >
        <Descriptions column={2} bordered style={{ marginBottom: 24 }}>
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
          <Descriptions.Item label="权重分布" span={2}>
            <MemoryWeightBadge
              weights={configResult.memory_config.memory_weights}
            />
          </Descriptions.Item>
          <Descriptions.Item label="效率提升">
            {(
              configResult.performance_prediction.efficiency_gain * 100
            ).toFixed(2)}
            %
          </Descriptions.Item>
          <Descriptions.Item label="连贯性提升">
            {configResult.performance_prediction.coherence_gain.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="资源成本">
            {configResult.performance_prediction.resource_cost.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="成本效益比">
            {configResult.performance_prediction.cost_benefit_ratio?.toFixed(
              2,
            ) || 'N/A'}
          </Descriptions.Item>
        </Descriptions>
      </ChartCard>
    )}
  </>
);

interface PredictionResultProps {
  prediction: API.PredictPerformanceResponse;
  loading: boolean;
}

const PredictionResult: React.FC<PredictionResultProps> = ({
  prediction,
  loading,
}) => (
  <ChartCard title="性能预测结果" loading={loading}>
    <Row gutter={[24, 24]}>
      <Col span={12}>
        <Descriptions column={1} bordered>
          <Descriptions.Item label="协同因子">
            {prediction.synergy_factor.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="衰减因子">
            {prediction.decay_factor.toFixed(2)}
          </Descriptions.Item>
        </Descriptions>
      </Col>
      <Col span={12}>
        <ChartCard title="各记忆层贡献度" height={200}>
          <Column
            data={[
              {
                type: 'STM',
                value: prediction.performance_breakdown.stm_contribution * 100,
              },
              {
                type: 'LTM',
                value: prediction.performance_breakdown.ltm_contribution * 100,
              },
              {
                type: 'KG',
                value: prediction.performance_breakdown.kg_contribution * 100,
              },
              {
                type: 'MM',
                value: prediction.performance_breakdown.mm_contribution * 100,
              },
            ]}
            xField="type"
            yField="value"
            label={{
              position: 'top',
              formatter: (d: any) => `${d.value.toFixed(2)}%`,
            }}
            height={200}
          />
        </ChartCard>
      </Col>
    </Row>
  </ChartCard>
);

// ── Page ──────────────────────────────────────────────────────────────────────

export default function MemoryConfigPage() {
  const [configResult, setConfigResult] =
    useState<API.SelectMemoryResponse | null>(null);
  const [predictionResult, setPredictionResult] =
    useState<API.PredictPerformanceResponse | null>(null);
  const [currentStatus, setCurrentStatus] =
    useState<API.MemoryStatusResponse | null>(null);
  const [configHistory, setConfigHistory] = useState<
    API.SelectMemoryResponse[]
  >([]);

  useRequest(getMemoryStatus, {
    onSuccess: (data: any) =>
      setCurrentStatus(data as API.MemoryStatusResponse),
  });

  const { loading: configLoading, run: selectConfig } = useRequest(
    selectMemoryConfig,
    {
      manual: true,
      onSuccess: (data: any) => {
        const d = data as API.SelectMemoryResponse;
        setConfigResult(d);
        setConfigHistory((prev) => [...prev, d].slice(-5));
        message.success('配置选择完成');
      },
      onError: () => message.error('配置选择失败'),
    },
  );

  const { loading: predictLoading, run: predict } = useRequest(
    predictPerformance,
    {
      manual: true,
      onSuccess: (data: any) => {
        setPredictionResult(data as API.PredictPerformanceResponse);
        message.success('性能预测完成');
      },
    },
  );

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
        user_id: DEFAULT_USER_ID,
        agent_id: DEFAULT_AGENT_ID,
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
    if (!configResult?.memory_config) {
      message.warning('请先选择记忆配置');
      return;
    }
    await predict({
      task_profile: {
        complexity: 0.75,
        modality_count: 2,
        temporal_scope: 'medium',
        reasoning_depth: 0.8,
        context_dependency: 0.6,
      },
      memory_config: configResult.memory_config,
    });
  };

  const comparisonData = useMemo(() => {
    if (!configResult?.memory_config || !currentStatus?.current_config)
      return [];
    return [
      {
        type: 'STM',
        value: currentStatus.current_config.memory_weights.stm,
        config: '当前配置',
      },
      {
        type: 'STM',
        value: configResult.memory_config.memory_weights.stm,
        config: '选中配置',
      },
      {
        type: 'LTM',
        value: currentStatus.current_config.memory_weights.ltm,
        config: '当前配置',
      },
      {
        type: 'LTM',
        value: configResult.memory_config.memory_weights.ltm,
        config: '选中配置',
      },
      {
        type: 'KG',
        value: currentStatus.current_config.memory_weights.kg,
        config: '当前配置',
      },
      {
        type: 'KG',
        value: configResult.memory_config.memory_weights.kg,
        config: '选中配置',
      },
      {
        type: 'MM',
        value: currentStatus.current_config.memory_weights.mm,
        config: '当前配置',
      },
      {
        type: 'MM',
        value: configResult.memory_config.memory_weights.mm,
        config: '选中配置',
      },
    ];
  }, [configResult, currentStatus]);

  const historyData = useMemo(
    () =>
      configHistory
        .filter((c) => c?.memory_config?.memory_weights)
        .flatMap((c, index) => [
          { index, value: c.memory_config.memory_weights.stm, type: 'STM' },
          { index, value: c.memory_config.memory_weights.ltm, type: 'LTM' },
          { index, value: c.memory_config.memory_weights.kg, type: 'KG' },
          { index, value: c.memory_config.memory_weights.mm, type: 'MM' },
        ]),
    [configHistory],
  );

  return (
    <PageContainer>
      <div style={{ marginBottom: 24 }}>
        <ConfigInputForm onSubmit={handleSelectConfig} />
      </div>

      {configResult && (
        <ConfigComparison
          comparisonData={comparisonData}
          historyData={historyData}
          configResult={configResult}
          loading={configLoading}
          onPredict={handlePredict}
        />
      )}

      {predictionResult && (
        <div style={{ marginTop: 24 }}>
          <PredictionResult
            prediction={predictionResult}
            loading={predictLoading}
          />
        </div>
      )}
    </PageContainer>
  );
}
