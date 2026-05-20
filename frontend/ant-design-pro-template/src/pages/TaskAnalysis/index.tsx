import { PageContainer, ProForm, ProFormText, ProFormSelect } from '@ant-design/pro-components';
import { Descriptions, Tag, Tabs, Row, Col, message } from 'antd';
import { Radar, Column } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { analyzeTaskCharacteristics } from '@/services/memory';
import { useState } from 'react';
import { ChartCard, MemoryWeightBadge } from '@/components/MemorySystem';
import { CHART_HEIGHT } from '@/config/appConfig';

// ── Sub-components ────────────────────────────────────────────────────────────

interface TaskInputFormProps {
  onSubmit: (values: any) => void;
  loading: boolean;
}

const TaskInputForm: React.FC<TaskInputFormProps> = ({ onSubmit, loading }) => (
  <ChartCard title="任务特征分析">
    <ProForm
      onFinish={onSubmit}
      submitter={{ searchConfig: { submitText: '开始分析' } }}
    >
      <ProFormText
        name="content"
        label="任务内容"
        placeholder="请输入任务内容"
        rules={[{ required: true, message: '请输入任务内容' }]}
        fieldProps={{ type: 'textarea', rows: 4 } as any}
      />
      <ProFormSelect
        name="modality"
        label="模态类型"
        mode="multiple"
        options={[
          { label: '文本', value: 'text' },
          { label: '图像', value: 'image' },
          { label: '音频', value: 'audio' },
          { label: '视频', value: 'video' },
        ]}
      />
      <ProFormText name="domain" label="领域" placeholder="可选" />
      <ProFormSelect
        name="complexity_hint"
        label="复杂度提示"
        options={[
          { label: '低', value: 'low' },
          { label: '中', value: 'medium' },
          { label: '高', value: 'high' },
        ]}
      />
      <ProFormSelect
        name="expected_duration"
        label="预期时长"
        options={[
          { label: '短', value: 'short' },
          { label: '中', value: 'medium' },
          { label: '长', value: 'long' },
        ]}
      />
    </ProForm>
  </ChartCard>
);

interface AnalysisResultPanelProps {
  result: API.AnalyzeTaskResponse;
  loading: boolean;
}

const AnalysisResultPanel: React.FC<AnalysisResultPanelProps> = ({ result, loading }) => {
  const radarData = [
    { item: '复杂度', value: result.characteristics.complexity * 100 },
    { item: '推理深度', value: result.characteristics.reasoning_depth * 100 },
    { item: '上下文依赖', value: result.characteristics.context_dependency * 100 },
    { item: '模态数量', value: (result.characteristics.modality_count / 4) * 100 },
  ];

  const strategyData = [
    { type: 'STM', enabled: result.memory_strategy.primary_memory === 'stm' ? 1 : 0, weight: 1.0 },
    { type: 'LTM', enabled: result.memory_strategy.secondary_memory.includes('ltm') ? 1 : 0, weight: 0.8 },
    { type: 'KG', enabled: result.memory_strategy.secondary_memory.includes('kg') ? 1 : 0, weight: 0.7 },
    { type: 'MM', enabled: result.memory_strategy.enable_multimodal ? 1 : 0, weight: 0.6 },
  ];

  const tabItems = [
    {
      key: 'radar',
      label: '雷达图',
      children: (
        <ChartCard title="任务特征雷达图" loading={loading} empty={radarData.length === 0} height={CHART_HEIGHT}>
          <Radar
            data={radarData}
            xField="item"
            yField="value"
            area={{}}
            point={{ size: 4 }}
            legend={false as any}
            yAxis={{ min: 0, max: 100 }}
            height={CHART_HEIGHT}
          />
        </ChartCard>
      ),
    },
    {
      key: 'strategy',
      label: '策略推荐',
      children: (
        <ChartCard title="记忆策略推荐" loading={loading} height={CHART_HEIGHT}>
          <Column
            data={strategyData}
            xField="type"
            yField="weight"
            colorField="enabled"
            color={({ enabled }: any) => (enabled === 1 ? '#6366f1' : '#d9d9d9')}
            columnStyle={{ radius: [4, 4, 0, 0] as any }}
            label={{
              position: 'top',
              formatter: (datum: any) => (datum.enabled === 1 ? `${datum.weight.toFixed(1)}` : '禁用'),
            }}
            height={CHART_HEIGHT}
          />
          <div style={{ marginTop: 16 }}>
            <p>
              <strong>主记忆:</strong>{' '}
              <Tag>{result.memory_strategy.primary_memory}</Tag>
            </p>
            <p>
              <strong>次记忆:</strong>{' '}
              {result.memory_strategy.secondary_memory.length > 0
                ? result.memory_strategy.secondary_memory.map((m) => (
                    <Tag key={m} style={{ marginRight: 8 }}>{m}</Tag>
                  ))
                : <Tag>无</Tag>}
            </p>
            <p>
              <strong>推理深度:</strong>{' '}
              <Tag>{result.memory_strategy.reasoning_depth}</Tag>
            </p>
            <p>
              <strong>多模态:</strong>{' '}
              <Tag color={result.memory_strategy.enable_multimodal ? 'green' : 'default'}>
                {result.memory_strategy.enable_multimodal ? '启用' : '禁用'}
              </Tag>
            </p>
          </div>
        </ChartCard>
      ),
    },
    {
      key: 'details',
      label: '详情数据',
      children: (
        <ChartCard title="分析结果详情" loading={loading}>
          <Descriptions column={2} bordered>
            <Descriptions.Item label="复杂度">
              {(result.characteristics.complexity * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="模态数量">
              {result.characteristics.modality_count}
            </Descriptions.Item>
            <Descriptions.Item label="时间范围">
              {result.characteristics.temporal_scope}
            </Descriptions.Item>
            <Descriptions.Item label="推理深度">
              {(result.characteristics.reasoning_depth * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="上下文依赖度">
              {(result.characteristics.context_dependency * 100).toFixed(2)}%
            </Descriptions.Item>
            <Descriptions.Item label="置信度">
              {(result.confidence_score * 100).toFixed(2)}%
            </Descriptions.Item>
          </Descriptions>
        </ChartCard>
      ),
    },
  ];

  return <Tabs items={tabItems} style={{ marginTop: 16 }} />;
};

// ── Page ──────────────────────────────────────────────────────────────────────

export default function TaskAnalysisPage() {
  const [analysisResult, setAnalysisResult] = useState<API.AnalyzeTaskResponse | null>(null);

  const { loading, run: analyzeTask } = useRequest(analyzeTaskCharacteristics, {
    manual: true,
    onSuccess: (data) => {
      setAnalysisResult(data);
      message.success('分析完成');
    },
    onError: () => {
      message.error('分析失败');
    },
  });

  const handleSubmit = async (values: any) => {
    const request: API.AnalyzeTaskRequest = {
      task_context: {
        content: values.content || '',
        modality: values.modality || [],
        context_history: [],
        task_metadata: values.domain
          ? {
              domain: values.domain,
              complexity_hint: values.complexity_hint,
              expected_duration: values.expected_duration,
            }
          : undefined,
      },
    };
    await analyzeTask(request);
  };

  return (
    <PageContainer>
      <TaskInputForm onSubmit={handleSubmit} loading={loading} />
      {analysisResult && (
        <AnalysisResultPanel result={analysisResult} loading={loading} />
      )}
    </PageContainer>
  );
}
