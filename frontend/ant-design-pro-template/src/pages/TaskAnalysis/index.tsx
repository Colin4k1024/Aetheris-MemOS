import { PageContainer, ProForm, ProFormText, ProFormSelect } from '@ant-design/pro-components';
import { Card, message, Descriptions, Tag, Row, Col, Table } from 'antd';
import { Radar, Column } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { analyzeTaskCharacteristics, batchAnalyzeCharacteristics } from '@/services/memory';
import { useState } from 'react';

export default function TaskAnalysisPage() {
  const [analysisResult, setAnalysisResult] = useState<API.AnalyzeTaskResponse | null>(null);
  const [batchResults, setBatchResults] = useState<API.BatchAnalyzeResponse | null>(null);

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

  const { loading: batchLoading, run: batchAnalyze } = useRequest(batchAnalyzeCharacteristics, {
    manual: true,
    onSuccess: (data) => {
      setBatchResults(data);
      message.success('批量分析完成');
    },
    onError: () => {
      message.error('批量分析失败');
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

  // 任务特征雷达图配置
  const radarConfig = analysisResult ? {
    data: [
      {
        item: '复杂度',
        value: analysisResult.characteristics.complexity * 100,
      },
      {
        item: '推理深度',
        value: analysisResult.characteristics.reasoning_depth * 100,
      },
      {
        item: '上下文依赖',
        value: analysisResult.characteristics.context_dependency * 100,
      },
      {
        item: '模态数量',
        value: (analysisResult.characteristics.modality_count / 4) * 100, // 归一化到0-100
      },
    ],
    xField: 'item',
    yField: 'value',
    area: {},
    point: { size: 4 },
    legend: false,
    yAxis: {
      min: 0,
      max: 100,
    },
  } : { data: [] };

  // 记忆策略推荐可视化
  const strategyConfig = analysisResult ? {
    data: [
      { type: 'STM', enabled: analysisResult.memory_strategy.primary_memory === 'stm' ? 1 : 0, weight: 1.0 },
      { type: 'LTM', enabled: analysisResult.memory_strategy.secondary_memory.includes('ltm') ? 1 : 0, weight: 0.8 },
      { type: 'KG', enabled: analysisResult.memory_strategy.secondary_memory.includes('kg') ? 1 : 0, weight: 0.7 },
      { type: 'MM', enabled: analysisResult.memory_strategy.enable_multimodal ? 1 : 0, weight: 0.6 },
    ],
    xField: 'type',
    yField: 'weight',
    colorField: 'enabled',
    color: ({ enabled }: any) => enabled === 1 ? '#1890ff' : '#d9d9d9',
    columnStyle: {
      radius: [4, 4, 0, 0],
    },
    label: {
      position: 'top' as const,
      formatter: (datum: any) => datum.enabled === 1 ? `${datum.weight.toFixed(1)}` : '禁用',
    },
  } : { data: [] };

  return (
    <PageContainer>
      <Card title="任务特征分析" style={{ marginBottom: 16 }}>
        <ProForm
          onFinish={handleSubmit}
          submitter={{
            searchConfig: {
              submitText: '开始分析',
            },
          }}
        >
          <ProFormText
            name="content"
            label="任务内容"
            placeholder="请输入任务内容"
            rules={[{ required: true, message: '请输入任务内容' }]}
            fieldProps={{
              type: 'textarea',
              rows: 4,
            }}
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
      </Card>

      {analysisResult && (
        <>
          <Row gutter={16} style={{ marginBottom: 16 }}>
            <Col span={12}>
              <Card title="任务特征雷达图" loading={loading}>
                <Radar {...radarConfig} />
              </Card>
            </Col>
            <Col span={12}>
              <Card title="记忆策略推荐" loading={loading}>
                <Column {...strategyConfig} />
                <div style={{ marginTop: 16 }}>
                  <p><strong>主记忆:</strong> <Tag>{analysisResult.memory_strategy.primary_memory}</Tag></p>
                  <p><strong>次记忆:</strong> {analysisResult.memory_strategy.secondary_memory.length > 0 ? (
                    analysisResult.memory_strategy.secondary_memory.map((m) => (
                      <Tag key={m} style={{ marginRight: 8 }}>{m}</Tag>
                    ))
                  ) : (
                    <Tag>无</Tag>
                  )}</p>
                  <p><strong>推理深度:</strong> <Tag>{analysisResult.memory_strategy.reasoning_depth}</Tag></p>
                  <p><strong>多模态:</strong> <Tag color={analysisResult.memory_strategy.enable_multimodal ? 'green' : 'default'}>
                    {analysisResult.memory_strategy.enable_multimodal ? '启用' : '禁用'}
                  </Tag></p>
                </div>
              </Card>
            </Col>
          </Row>

          <Card title="分析结果详情" loading={loading}>
            <Descriptions column={2} bordered>
              <Descriptions.Item label="复杂度">
                {(analysisResult.characteristics.complexity * 100).toFixed(2)}%
              </Descriptions.Item>
              <Descriptions.Item label="模态数量">
                {analysisResult.characteristics.modality_count}
              </Descriptions.Item>
              <Descriptions.Item label="时间范围">
                {analysisResult.characteristics.temporal_scope}
              </Descriptions.Item>
              <Descriptions.Item label="推理深度">
                {(analysisResult.characteristics.reasoning_depth * 100).toFixed(2)}%
              </Descriptions.Item>
              <Descriptions.Item label="上下文依赖度">
                {(analysisResult.characteristics.context_dependency * 100).toFixed(2)}%
              </Descriptions.Item>
              <Descriptions.Item label="置信度">
                {(analysisResult.confidence_score * 100).toFixed(2)}%
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </>
      )}
    </PageContainer>
  );
}
