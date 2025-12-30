import { PageContainer, ProForm, ProFormSlider } from '@ant-design/pro-components';
import { Card, message, Descriptions, Tag, Alert, List, Row, Col } from 'antd';
import { Line, Area } from '@ant-design/charts';
import { useRequest } from '@umijs/max';
import { getResources, calculateCostBenefit, optimize } from '@/services/memory';
import { useState, useEffect } from 'react';

export default function ResourceMonitorPage() {
  const [resourceStatus, setResourceStatus] = useState<API.CurrentResourceStatus | null>(null);
  const [costBenefit, setCostBenefit] = useState<API.CostBenefitResponse | null>(null);
  const [optimization, setOptimization] = useState<API.OptimizationResult | null>(null);
  const [resourceHistory, setResourceHistory] = useState<any[]>([]);

  const { loading: resourcesLoading, run: fetchResources } = useRequest(getResources, {
    onSuccess: (data) => {
      setResourceStatus(data);
      // 记录资源使用历史
      if (data) {
        const now = Date.now();
        setResourceHistory(prev => {
          const newHistory = [...prev, {
            time: now,
            memory: data.current_status.memory_usage_percent,
            cpu: data.current_status.cpu_usage_percent,
            responseTime: data.current_status.response_time_ms,
          }];
          // 只保留最近20个数据点
          return newHistory.slice(-20);
        });
      }
    },
  });

  const { loading: costBenefitLoading, run: calculateCostBenefitRatio } = useRequest(
    calculateCostBenefit,
    {
      manual: true,
      onSuccess: (data) => {
        setCostBenefit(data);
        message.success('成本效益分析完成');
      },
    },
  );

  const { loading: optimizeLoading, run: runOptimize } = useRequest(optimize, {
    manual: true,
    onSuccess: (data) => {
      setOptimization(data);
      message.success('优化建议生成完成');
    },
  });

  const handleCalculateCostBenefit = async (values: any) => {
    if (!resourceStatus) {
      message.warning('请先获取资源状态');
      return;
    }
    const request: API.CostBenefitRequest = {
      performance_prediction: {
        efficiency_gain: values.efficiency || 0.4,
        coherence_gain: values.coherence || 1.5,
        resource_cost: 0.65,
        cost_benefit_ratio: undefined,
        confidence_score: undefined,
      },
      resource_status: resourceStatus.current_status,
    };
    await calculateCostBenefitRatio(request);
  };

  const handleOptimize = async (values: any) => {
    if (!resourceStatus) {
      message.warning('请先获取资源状态');
      return;
    }
    const request: API.OptimizeRequest = {
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
    };
    await runOptimize(request);
  };

  useEffect(() => {
    fetchResources();
    const interval = setInterval(() => {
      fetchResources();
    }, 5000); // 每5秒刷新一次
    return () => clearInterval(interval);
  }, []);

  // 资源使用趋势图配置
  const resourceTrendConfig = {
    data: resourceHistory.flatMap(item => [
      { time: item.time, value: item.memory, type: '内存使用率 (%)' },
      { time: item.time, value: item.cpu, type: 'CPU 使用率 (%)' },
    ]),
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
          const date = new Date(parseInt(text));
          return `${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}:${date.getSeconds().toString().padStart(2, '0')}`;
        },
      },
    },
    yAxis: {
      label: {
        formatter: (text: string) => `${text}%`,
      },
    },
  };

  // 响应时间趋势图配置
  const responseTimeConfig = {
    data: resourceHistory.map(item => ({
      time: item.time,
      value: item.responseTime,
    })),
    xField: 'time',
    yField: 'value',
    smooth: true,
    area: {
      style: {
        fill: 'l(270) 0:#ffffff 0.5:#7ec2f3 1:#1890ff',
      },
    },
    point: { size: 3 },
    xAxis: {
      type: 'time',
      label: {
        formatter: (text: string) => {
          const date = new Date(parseInt(text));
          return `${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}:${date.getSeconds().toString().padStart(2, '0')}`;
        },
      },
    },
    yAxis: {
      label: {
        formatter: (text: string) => `${text}ms`,
      },
    },
  };

  return (
    <PageContainer>
      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={24}>
          <Card title="资源使用状态" loading={resourcesLoading}>
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
                    style={{ marginBottom: 16 }}
                  />
                )}
                <Descriptions column={2} bordered>
                  <Descriptions.Item label="状态">
                    <Tag
                      color={
                        resourceStatus.status === 'healthy'
                          ? 'green'
                          : resourceStatus.status === 'warning'
                          ? 'orange'
                          : 'red'
                      }
                    >
                      {resourceStatus.status}
                    </Tag>
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
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={12}>
          <Card title="资源使用趋势（实时监控）" loading={resourcesLoading}>
            {resourceHistory.length > 0 && <Line {...resourceTrendConfig} />}
          </Card>
        </Col>
        <Col span={12}>
          <Card title="响应时间趋势" loading={resourcesLoading}>
            {resourceHistory.length > 0 && <Area {...responseTimeConfig} />}
          </Card>
        </Col>
      </Row>

      <Card title="成本效益分析" style={{ marginBottom: 16 }}>
        <ProForm
          onFinish={handleCalculateCostBenefit}
          submitter={{
            searchConfig: {
              submitText: '计算成本效益比',
            },
          }}
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
              <Tag
                color={
                  costBenefit.recommendation === 'optimal'
                    ? 'green'
                    : costBenefit.recommendation === 'suboptimal'
                    ? 'orange'
                    : 'red'
                }
              >
                {costBenefit.recommendation}
              </Tag>
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
      </Card>

      <Card title="资源优化建议">
        <ProForm
          onFinish={handleOptimize}
          submitter={{
            searchConfig: {
              submitText: '生成优化建议',
            },
          }}
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
            <h3>优化建议</h3>
            <List
              dataSource={optimization.optimization_suggestions}
              renderItem={(item) => (
                <List.Item>
                  <List.Item.Meta
                    title={item.description}
                    description={
                      <>
                        <Tag color={item.risk_level === 'low' ? 'green' : 'orange'}>
                          {item.risk_level}
                        </Tag>
                        预期改进: {(item.expected_improvement * 100).toFixed(1)}%
                      </>
                    }
                  />
                </List.Item>
              )}
            />
            <h3 style={{ marginTop: 16 }}>优化后的配置</h3>
            <Descriptions column={2} bordered>
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
            <h3 style={{ marginTop: 16 }}>预期改进</h3>
            <Descriptions column={3} bordered>
              <Descriptions.Item label="效率提升">
                {(optimization.predicted_improvement.efficiency_gain * 100).toFixed(2)}%
              </Descriptions.Item>
              <Descriptions.Item label="连贯性提升">
                {optimization.predicted_improvement.coherence_gain.toFixed(2)}
              </Descriptions.Item>
              <Descriptions.Item label="资源成本降低">
                {(optimization.predicted_improvement.resource_cost_reduction * 100).toFixed(2)}%
              </Descriptions.Item>
            </Descriptions>
          </div>
        )}
      </Card>
    </PageContainer>
  );
}
