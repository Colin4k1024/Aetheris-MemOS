import { Area, Line } from '@ant-design/charts';
import {
  PageContainer,
  ProForm,
  ProFormSlider,
} from '@ant-design/pro-components';
import { useRequest } from '@umijs/max';
import { Alert, Col, Descriptions, List, message, Row } from 'antd';
import React, { useState } from 'react';
import { ChartCard, StatusTag } from '@/components/MemorySystem';
import { CHART_HEIGHT, POLLING_INTERVALS } from '@/config/appConfig';
import usePolling from '@/hooks/usePolling';
import {
  calculateCostBenefit,
  getResources,
  optimize,
} from '@/services/memory';
import { formatTimeWithSeconds } from '@/utils/formatters';

// ── Sub-components ────────────────────────────────────────────────────────────

interface CostBenefitSectionProps {
  resourceStatus: API.CurrentResourceStatus | null;
}

const CostBenefitSection: React.FC<CostBenefitSectionProps> = ({
  resourceStatus,
}) => {
  const [costBenefit, setCostBenefit] =
    useState<API.CostBenefitResponse | null>(null);

  const { run: calculateCostBenefitRatio } = useRequest(calculateCostBenefit, {
    manual: true,
    onSuccess: (data: any) => {
      setCostBenefit(data as API.CostBenefitResponse);
      message.success('成本效益分析完成');
    },
  });

  const handleSubmit = async (values: any) => {
    if (!resourceStatus) {
      message.warning('请先获取资源状态');
      return;
    }
    await calculateCostBenefitRatio({
      performance_prediction: {
        efficiency_gain: values.efficiency || 0.4,
        coherence_gain: values.coherence || 1.5,
        resource_cost: 0.65,
        cost_benefit_ratio: undefined,
        confidence_score: undefined,
      },
      resource_status: resourceStatus.current_status,
    });
  };

  return (
    <ChartCard title="成本效益分析">
      <ProForm
        onFinish={handleSubmit}
        submitter={{ searchConfig: { submitText: '计算成本效益比' } }}
      >
        <ProFormSlider
          name="efficiency"
          label="效率增益"
          min={0}
          max={1}
          step={0.01}
          initialValue={0.4}
        />
        <ProFormSlider
          name="coherence"
          label="连贯性增益"
          min={0}
          max={3}
          step={0.1}
          initialValue={1.5}
        />
      </ProForm>

      {costBenefit && (
        <Descriptions column={2} bordered style={{ marginTop: 16 }}>
          <Descriptions.Item label="成本效益比">
            {costBenefit.cost_benefit_ratio.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="性能得分">
            {costBenefit.performance_score.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="资源成本">
            {costBenefit.resource_cost.toFixed(2)}
          </Descriptions.Item>
          <Descriptions.Item label="推荐">
            <StatusTag status={costBenefit.recommendation} />
          </Descriptions.Item>
          {costBenefit.optimization_suggestions.length > 0 && (
            <Descriptions.Item label="优化建议" span={2}>
              <List
                size="small"
                dataSource={costBenefit.optimization_suggestions}
                renderItem={(item) => <List.Item>{item}</List.Item>}
              />
            </Descriptions.Item>
          )}
        </Descriptions>
      )}
    </ChartCard>
  );
};

interface OptimizeSectionProps {
  resourceStatus: API.CurrentResourceStatus | null;
}

const OptimizeSection: React.FC<OptimizeSectionProps> = ({
  resourceStatus,
}) => {
  const [optimization, setOptimization] =
    useState<API.OptimizationResult | null>(null);

  const { run: runOptimize } = useRequest(optimize, {
    manual: true,
    onSuccess: (data: any) => {
      setOptimization(data as API.OptimizationResult);
      message.success('优化建议生成完成');
    },
  });

  const handleSubmit = async (values: any) => {
    if (!resourceStatus) {
      message.warning('请先获取资源状态');
      return;
    }
    await runOptimize({
      current_config: {
        primary_memory: 'stm',
        secondary_memory: ['ltm'],
        memory_weights: {
          stm: 1.0,
          ltm: values.ltm_weight || 0.8,
          kg: values.kg_weight || 0.0,
          mm: 0.0,
        },
        reasoning_depth: 'medium',
        enable_multimodal: false,
      },
      performance_goals: {
        target_efficiency: values.target_efficiency || 0.4,
        target_coherence: values.target_coherence || 1.5,
        max_resource_cost: values.max_resource_cost || 0.7,
      },
    });
  };

  return (
    <ChartCard title="资源优化建议">
      <ProForm
        onFinish={handleSubmit}
        submitter={{ searchConfig: { submitText: '生成优化建议' } }}
      >
        <ProFormSlider
          name="ltm_weight"
          label="LTM 权重"
          min={0}
          max={1}
          step={0.1}
          initialValue={0.8}
        />
        <ProFormSlider
          name="kg_weight"
          label="KG 权重"
          min={0}
          max={1}
          step={0.1}
          initialValue={0.0}
        />
        <ProFormSlider
          name="target_efficiency"
          label="目标效率"
          min={0}
          max={1}
          step={0.01}
          initialValue={0.4}
        />
        <ProFormSlider
          name="target_coherence"
          label="目标连贯性"
          min={0}
          max={3}
          step={0.1}
          initialValue={1.5}
        />
        <ProFormSlider
          name="max_resource_cost"
          label="最大资源成本"
          min={0}
          max={1}
          step={0.1}
          initialValue={0.7}
        />
      </ProForm>

      {optimization && (
        <div style={{ marginTop: 16 }}>
          <List
            header={<strong>优化建议</strong>}
            dataSource={optimization.optimization_suggestions}
            renderItem={(item) => (
              <List.Item>
                <List.Item.Meta
                  title={item.description}
                  description={
                    <>
                      <StatusTag status={item.risk_level} />
                      预期改进: {(item.expected_improvement * 100).toFixed(1)}%
                    </>
                  }
                />
              </List.Item>
            )}
          />
          <Descriptions
            column={2}
            bordered
            style={{ marginTop: 16 }}
            title="优化后的配置"
          >
            <Descriptions.Item label="STM 权重">
              {optimization.optimized_config.memory_weights.stm.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="LTM 权重">
              {optimization.optimized_config.memory_weights.ltm.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="KG 权重">
              {optimization.optimized_config.memory_weights.kg.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="MM 权重">
              {optimization.optimized_config.memory_weights.mm.toFixed(2)}
            </Descriptions.Item>
          </Descriptions>
          <Descriptions
            column={3}
            bordered
            style={{ marginTop: 16 }}
            title="预期改进"
          >
            <Descriptions.Item label="效率提升">
              {(
                optimization.predicted_improvement.efficiency_gain * 100
              ).toFixed(2)}
              %
            </Descriptions.Item>
            <Descriptions.Item label="连贯性提升">
              {optimization.predicted_improvement.coherence_gain.toFixed(2)}
            </Descriptions.Item>
            <Descriptions.Item label="资源成本降低">
              {(
                optimization.predicted_improvement.resource_cost_reduction * 100
              ).toFixed(2)}
              %
            </Descriptions.Item>
          </Descriptions>
        </div>
      )}
    </ChartCard>
  );
};

// ── Page ──────────────────────────────────────────────────────────────────────

export default function ResourceMonitorPage() {
  const [resourceStatus, setResourceStatus] =
    useState<API.CurrentResourceStatus | null>(null);
  const [resourceHistory, setResourceHistory] = useState<any[]>([]);

  const { loading: resourcesLoading, run: fetchResources } = useRequest(
    getResources,
    {
      manual: true,
      formatResult: (r: any) => r,
      onSuccess: (data: any) => {
        const d = data as API.CurrentResourceStatus;
        setResourceStatus(d);
        if (d?.current_status) {
          const now = Date.now();
          setResourceHistory((prev) => {
            const next = [
              ...prev,
              {
                time: now,
                memory: d.current_status.memory_usage_percent,
                cpu: d.current_status.cpu_usage_percent,
                responseTime: d.current_status.response_time_ms,
              },
            ];
            return next.slice(-20);
          });
        }
      },
    },
  );

  usePolling(fetchResources, { interval: POLLING_INTERVALS.NORMAL });

  const resourceTrendData = resourceHistory.flatMap((item) => [
    { time: item.time, value: item.memory, type: '内存使用率 (%)' },
    { time: item.time, value: item.cpu, type: 'CPU 使用率 (%)' },
  ]);

  const responseTimeData = resourceHistory.map((item) => ({
    time: item.time,
    value: item.responseTime,
  }));

  return (
    <PageContainer>
      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={24}>
          <ChartCard
            title="资源使用状态"
            loading={resourcesLoading}
            empty={!resourceStatus}
          >
            {resourceStatus && (
              <>
                {resourceStatus.alerts.length > 0 && (
                  <Alert
                    message="资源告警"
                    description={
                      <List
                        size="small"
                        dataSource={resourceStatus.alerts}
                        renderItem={(item) => <List.Item>{item}</List.Item>}
                      />
                    }
                    type="warning"
                    style={{ marginBottom: 24 }}
                  />
                )}
                <Descriptions column={2} bordered>
                  <Descriptions.Item label="状态">
                    <StatusTag status={resourceStatus.status} />
                  </Descriptions.Item>
                  <Descriptions.Item label="内存使用">
                    {resourceStatus.current_status.memory_usage_mb} MB (
                    {resourceStatus.current_status.memory_usage_percent}%)
                  </Descriptions.Item>
                  <Descriptions.Item label="CPU 使用率">
                    {resourceStatus.current_status.cpu_usage_percent}%
                  </Descriptions.Item>
                  <Descriptions.Item label="响应时间">
                    {resourceStatus.current_status.response_time_ms} ms
                  </Descriptions.Item>
                  <Descriptions.Item label="存储使用率">
                    {resourceStatus.current_status.storage_usage_percent}%
                  </Descriptions.Item>
                  <Descriptions.Item label="内存限制">
                    {resourceStatus.resource_limits.memory_limit_mb} MB
                  </Descriptions.Item>
                  <Descriptions.Item label="CPU 限制">
                    {resourceStatus.resource_limits.cpu_limit_percent}%
                  </Descriptions.Item>
                  <Descriptions.Item label="响应时间限制">
                    {resourceStatus.resource_limits.response_time_limit_ms} ms
                  </Descriptions.Item>
                </Descriptions>
              </>
            )}
          </ChartCard>
        </Col>
      </Row>

      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={12}>
          <ChartCard
            title="资源使用趋势（实时监控）"
            loading={resourcesLoading}
            empty={resourceTrendData.length === 0}
            height={CHART_HEIGHT}
          >
            <Line
              data={resourceTrendData}
              xField="time"
              yField="value"
              seriesField="type"
              smooth
              point={{ size: 3 }}
              legend={{ position: 'top' }}
              xAxis={{
                type: 'time',
                label: { formatter: formatTimeWithSeconds },
              }}
              yAxis={{ label: { formatter: (t: string) => `${t}%` } }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
        <Col span={12}>
          <ChartCard
            title="响应时间趋势"
            loading={resourcesLoading}
            empty={responseTimeData.length === 0}
            height={CHART_HEIGHT}
          >
            <Area
              data={responseTimeData}
              xField="time"
              yField="value"
              smooth
              areaStyle={{ fill: 'l(270) 0:#ffffff 0.5:#7ec2f3 1:#6366f1' }}
              point={{ size: 3 }}
              xAxis={{
                type: 'time',
                label: { formatter: formatTimeWithSeconds },
              }}
              yAxis={{ label: { formatter: (t: string) => `${t}ms` } }}
              height={CHART_HEIGHT}
            />
          </ChartCard>
        </Col>
      </Row>

      <div style={{ marginBottom: 24 }}>
        <CostBenefitSection resourceStatus={resourceStatus} />
      </div>
      <OptimizeSection resourceStatus={resourceStatus} />
    </PageContainer>
  );
}
