import { PageContainer, ProForm, ProFormSelect, ProFormSlider, ProFormText } from '@ant-design/pro-components';
import { Card, message, Descriptions, Steps, Tag, Spin, Table } from 'antd';
import { useRequest } from '@umijs/max';
import { getDecisionTrace } from '@/services/memory';
import { useState } from 'react';

export default function MemoryDecisionTracePage() {
  const [trace, setTrace] = useState<API.DecisionTrace | null>(null);

  const { loading, run: fetchTrace } = useRequest(getDecisionTrace, {
    manual: true,
    onSuccess: (data) => {
      setTrace(data);
      message.success('决策链路获取成功');
    },
    onError: () => {
      message.error('获取决策链路失败');
    },
  });

  const handleSubmit = async (values: Record<string, any>) => {
    const modalityMap: Record<string, API.Modality> = {
      text: 'text',
      image: 'image',
      audio: 'audio',
      video: 'video',
    };
    const request: API.SelectMemoryRequest = {
      task_context: {
        task_id: values.task_id || `task_${Date.now()}`,
        task_type: values.task_type || 'query',
        complexity: Number(values.complexity) ?? 0.5,
        modality_requirements: (values.modality_requirements || ['text']).map((m: string) => modalityMap[m] || 'text'),
        temporal_scope: values.temporal_scope || 'medium',
        reasoning_depth: values.reasoning_depth || 'medium',
        context_dependency: Number(values.context_dependency) ?? 0.5,
        user_id: values.user_id || 'user_1',
        agent_id: values.agent_id || 'agent_1',
      },
      resource_constraints: {
        max_memory_usage_mb: Number(values.max_memory_usage_mb) || 1024,
        max_cpu_usage_percent: Number(values.max_cpu_usage_percent) || 80,
        max_response_time_ms: Number(values.max_response_time_ms) || 2000,
        storage_quota_percent: Number(values.storage_quota_percent) || 90,
      },
      preferences: {
        prioritize_efficiency: values.prioritize_efficiency !== false,
        prioritize_coherence: values.prioritize_coherence === true,
        enable_multimodal: values.enable_multimodal === true,
        enable_reasoning: values.enable_reasoning !== false,
      },
    };
    await fetchTrace(request);
  };

  return (
    <PageContainer>
      <Card title="记忆决策链路追踪" style={{ marginBottom: 16 }}>
        <p style={{ color: 'var(--ant-color-text-secondary)', marginBottom: 16 }}>
          输入任务上下文与约束，查看系统完整的决策过程：Analyzer → Predictor → Weight Adjuster → 最终配置（不落库）。
        </p>
        <ProForm
          onFinish={handleSubmit}
          submitter={{
            searchConfig: {
              submitText: '获取决策链路',
            },
          }}
          layout="horizontal"
          initialValues={{
            task_id: `task_${Date.now()}`,
            task_type: 'query',
            complexity: 0.6,
            temporal_scope: 'medium',
            reasoning_depth: 'medium',
            context_dependency: 0.5,
            user_id: 'user_1',
            agent_id: 'agent_1',
            modality_requirements: ['text'],
            max_memory_usage_mb: 1024,
            max_cpu_usage_percent: 80,
            max_response_time_ms: 2000,
            storage_quota_percent: 90,
            prioritize_efficiency: true,
            enable_reasoning: true,
          }}
        >
          <ProFormText name="task_id" label="任务 ID" placeholder="task_001" />
          <ProFormSelect
            name="task_type"
            label="任务类型"
            options={[
              { value: 'conversation', label: 'Conversation' },
              { value: 'task', label: 'Task' },
              { value: 'query', label: 'Query' },
            ]}
          />
          <ProFormSlider name="complexity" label="复杂度" min={0} max={1} step={0.1} />
          <ProFormSelect
            name="modality_requirements"
            label="模态需求"
            mode="multiple"
            options={[
              { value: 'text', label: 'Text' },
              { value: 'image', label: 'Image' },
              { value: 'audio', label: 'Audio' },
              { value: 'video', label: 'Video' },
            ]}
          />
          <ProFormSelect
            name="temporal_scope"
            label="时间范围"
            options={[
              { value: 'short', label: 'Short' },
              { value: 'medium', label: 'Medium' },
              { value: 'long', label: 'Long' },
            ]}
          />
          <ProFormSelect
            name="reasoning_depth"
            label="推理深度"
            options={[
              { value: 'shallow', label: 'Shallow' },
              { value: 'medium', label: 'Medium' },
              { value: 'deep', label: 'Deep' },
            ]}
          />
          <ProFormSlider name="context_dependency" label="上下文依赖" min={0} max={1} step={0.1} />
          <ProFormText name="user_id" label="User ID" />
          <ProFormText name="agent_id" label="Agent ID" />
        </ProForm>
      </Card>

      {loading && (
        <Card>
          <Spin tip="正在执行决策链路..." />
        </Card>
      )}

      {!loading && trace && (
        <>
          <Card title="决策路径图" style={{ marginBottom: 16 }}>
            <div style={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 4 }}>
              {['Analyzer', 'Resource', 'Initial Config', 'Predictor', 'Weight Adjuster', 'Result'].map((label, i) => (
                <span key={label}>
                  <Tag color={i === 0 ? 'blue' : i === 5 ? 'green' : 'default'}>{label}</Tag>
                  {i < 5 && <span style={{ margin: '0 4px', color: 'var(--ant-color-text-tertiary)' }}>→</span>}
                </span>
              ))}
            </div>
          </Card>

          {trace.memory_contributions && trace.memory_contributions.length > 0 && (
            <Card title="Task → Memory 选择映射" style={{ marginBottom: 16 }}>
              <Table
                size="small"
                rowKey="memory_type"
                pagination={false}
                dataSource={trace.memory_contributions}
                columns={[
                  { title: '记忆类型', dataIndex: 'memory_type', key: 'memory_type', render: (t: string) => <Tag>{t.toUpperCase()}</Tag> },
                  { title: '权重', dataIndex: 'weight', key: 'weight', render: (w: number) => w.toFixed(2) },
                  { title: '原因', dataIndex: 'reason', key: 'reason', ellipsis: true },
                ]}
              />
            </Card>
          )}

          <Card title={`决策链路：${trace.task_id}`}>
          <Steps
            direction="vertical"
            current={4}
            items={[
              {
                title: '1. Analyzer 输出',
                description: (
                  <Descriptions size="small" column={2} bordered>
                    <Descriptions.Item label="复杂度">{trace.analyzer.task_characteristics.complexity.toFixed(2)}</Descriptions.Item>
                    <Descriptions.Item label="模态数">{trace.analyzer.task_characteristics.modality_count}</Descriptions.Item>
                    <Descriptions.Item label="时间范围">{trace.analyzer.task_characteristics.temporal_scope}</Descriptions.Item>
                    <Descriptions.Item label="推理深度">{trace.analyzer.task_characteristics.reasoning_depth.toFixed(2)}</Descriptions.Item>
                    <Descriptions.Item label="主记忆">{trace.analyzer.memory_strategy.primary_memory}</Descriptions.Item>
                    <Descriptions.Item label="次记忆">{trace.analyzer.memory_strategy.secondary_memory.join(', ') || '-'}</Descriptions.Item>
                    <Descriptions.Item label="置信度">{trace.analyzer.confidence_score.toFixed(2)}</Descriptions.Item>
                  </Descriptions>
                ),
              },
              {
                title: '2. 资源状态',
                description: (
                  <Descriptions size="small" column={2} bordered>
                    <Descriptions.Item label="状态">{trace.resource_status.status}</Descriptions.Item>
                    <Descriptions.Item label="内存">{trace.resource_status.current_status.memory_usage_mb} MB</Descriptions.Item>
                    <Descriptions.Item label="CPU">{trace.resource_status.current_status.cpu_usage_percent}%</Descriptions.Item>
                    <Descriptions.Item label="响应时间">{trace.resource_status.current_status.response_time_ms} ms</Descriptions.Item>
                    {trace.resource_status.alerts?.length > 0 && (
                      <Descriptions.Item label="告警" span={2}>{trace.resource_status.alerts.join('; ')}</Descriptions.Item>
                    )}
                  </Descriptions>
                ),
              },
              {
                title: '3. 初始记忆配置',
                description: (
                  <Descriptions size="small" column={2} bordered>
                    <Descriptions.Item label="主记忆">{trace.initial_memory_config.primary_memory}</Descriptions.Item>
                    <Descriptions.Item label="次记忆">{trace.initial_memory_config.secondary_memory.join(', ') || '-'}</Descriptions.Item>
                    <Descriptions.Item label="STM">{trace.initial_memory_config.memory_weights.stm.toFixed(2)}</Descriptions.Item>
                    <Descriptions.Item label="LTM">{trace.initial_memory_config.memory_weights.ltm.toFixed(2)}</Descriptions.Item>
                    <Descriptions.Item label="KG">{trace.initial_memory_config.memory_weights.kg.toFixed(2)}</Descriptions.Item>
                    <Descriptions.Item label="MM">{trace.initial_memory_config.memory_weights.mm.toFixed(2)}</Descriptions.Item>
                  </Descriptions>
                ),
              },
              {
                title: '4. Predictor 评估',
                description: (
                  <Descriptions size="small" column={2} bordered>
                    <Descriptions.Item label="效率增益">{trace.predictor.performance_prediction.efficiency_gain.toFixed(3)}</Descriptions.Item>
                    <Descriptions.Item label="连贯增益">{trace.predictor.performance_prediction.coherence_gain.toFixed(3)}</Descriptions.Item>
                    <Descriptions.Item label="协同因子">{trace.predictor.synergy_factor.toFixed(3)}</Descriptions.Item>
                    <Descriptions.Item label="衰减因子">{trace.predictor.decay_factor.toFixed(3)}</Descriptions.Item>
                  </Descriptions>
                ),
              },
              {
                title: '5. 成本效益比与权重调整',
                description: (
                  <>
                    <p><Tag color="blue">成本效益比: {trace.cost_benefit_ratio.toFixed(2)}</Tag></p>
                    <Descriptions size="small" column={2} bordered>
                      <Descriptions.Item label="调整后 STM">{trace.weight_adjustment.adjusted_weights.stm.toFixed(2)}</Descriptions.Item>
                      <Descriptions.Item label="调整后 LTM">{trace.weight_adjustment.adjusted_weights.ltm.toFixed(2)}</Descriptions.Item>
                      <Descriptions.Item label="调整后 KG">{trace.weight_adjustment.adjusted_weights.kg.toFixed(2)}</Descriptions.Item>
                      <Descriptions.Item label="调整后 MM">{trace.weight_adjustment.adjusted_weights.mm.toFixed(2)}</Descriptions.Item>
                      <Descriptions.Item label="LTM 原因" span={2}>{trace.weight_adjustment.adjustment_reasons.ltm || '-'}</Descriptions.Item>
                      <Descriptions.Item label="KG 原因" span={2}>{trace.weight_adjustment.adjustment_reasons.kg || '-'}</Descriptions.Item>
                      <Descriptions.Item label="MM 原因" span={2}>{trace.weight_adjustment.adjustment_reasons.mm || '-'}</Descriptions.Item>
                    </Descriptions>
                  </>
                ),
              },
              {
                title: '6. 最终结果',
                description: (
                  <Descriptions size="small" column={2} bordered>
                    <Descriptions.Item label="预估内存">{trace.final_result.resource_requirements.estimated_memory_mb} MB</Descriptions.Item>
                    <Descriptions.Item label="预估 CPU">{trace.final_result.resource_requirements.estimated_cpu_percent}%</Descriptions.Item>
                    <Descriptions.Item label="预估响应">{trace.final_result.resource_requirements.estimated_response_time_ms} ms</Descriptions.Item>
                    <Descriptions.Item label="效率">{trace.final_result.performance_prediction.efficiency_gain.toFixed(3)}</Descriptions.Item>
                    <Descriptions.Item label="连贯">{trace.final_result.performance_prediction.coherence_gain.toFixed(3)}</Descriptions.Item>
                    <Descriptions.Item label="成本">{trace.final_result.performance_prediction.resource_cost.toFixed(3)}</Descriptions.Item>
                  </Descriptions>
                ),
              },
            ]}
          />
          </Card>
        </>
      )}
    </PageContainer>
  );
}
