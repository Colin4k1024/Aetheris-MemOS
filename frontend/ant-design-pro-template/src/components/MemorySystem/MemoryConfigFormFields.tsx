import React from 'react';
import { Space, Typography } from 'antd';
import {
  ProFormSwitch,
  ProFormDigit,
  ProFormTextArea,
} from '@ant-design/pro-components';

const { Title } = Typography;

interface MemoryConfigFormFieldsProps {
  /** Show default values when creating; omit to leave fields empty (edit mode) */
  withDefaults?: boolean;
}

const MemoryConfigFormFields: React.FC<MemoryConfigFormFieldsProps> = ({
  withDefaults = false,
}) => {
  return (
    <>
      <Space orientation="vertical" style={{ width: '100%' }}>
        <Title level={5}>短期记忆 (STM)</Title>
        <ProFormSwitch name="stmEnabled" label="启用 STM" />
        <ProFormDigit
          name="stmMaxLength"
          label="最大长度"
          min={1}
          initialValue={withDefaults ? 4096 : undefined}
        />
        <ProFormDigit
          name="stmRetentionHours"
          label="保留时间 (小时)"
          min={1}
          initialValue={withDefaults ? 24 : undefined}
        />
      </Space>

      <Space orientation="vertical" style={{ width: '100%' }}>
        <Title level={5}>长期记忆 (LTM)</Title>
        <ProFormSwitch name="ltmEnabled" label="启用 LTM" />
        <ProFormDigit
          name="ltmMaxEntries"
          label="最大条目数"
          min={1}
          initialValue={withDefaults ? 10000 : undefined}
        />
        <ProFormDigit
          name="ltmQualityThreshold"
          label="质量阈值"
          min={0}
          max={1}
          step={0.01}
          initialValue={withDefaults ? 0.5 : undefined}
        />
      </Space>

      <Space orientation="vertical" style={{ width: '100%' }}>
        <Title level={5}>知识图谱 (KG)</Title>
        <ProFormSwitch name="kgEnabled" label="启用 KG" />
        <ProFormDigit
          name="kgMaxEntities"
          label="最大实体数"
          min={1}
          initialValue={withDefaults ? 1000 : undefined}
        />
        <ProFormDigit
          name="kgConfidenceThreshold"
          label="置信度阈值"
          min={0}
          max={1}
          step={0.01}
          initialValue={withDefaults ? 0.7 : undefined}
        />
      </Space>

      <Space orientation="vertical" style={{ width: '100%' }}>
        <Title level={5}>多模态记忆 (MM)</Title>
        <ProFormSwitch name="mmEnabled" label="启用 MM" />
        <ProFormDigit
          name="mmMaxEntries"
          label="最大条目数"
          min={1}
          initialValue={withDefaults ? 1000 : undefined}
        />
        <ProFormTextArea name="mmModalityTypes" label="模态类型" />
      </Space>

      <Space orientation="vertical" style={{ width: '100%' }}>
        <Title level={5}>性能限制</Title>
        <ProFormDigit
          name="maxResponseTimeMs"
          label="最大响应时间 (ms)"
          min={1}
          initialValue={withDefaults ? 2000 : undefined}
        />
        <ProFormDigit
          name="maxMemoryUsageMb"
          label="最大内存使用 (MB)"
          min={1}
          initialValue={withDefaults ? 1024 : undefined}
        />
        <ProFormDigit
          name="maxCpuUsagePercent"
          label="最大 CPU 使用率 (%)"
          min={1}
          max={100}
          initialValue={withDefaults ? 80 : undefined}
        />
      </Space>
    </>
  );
};

export default MemoryConfigFormFields;
