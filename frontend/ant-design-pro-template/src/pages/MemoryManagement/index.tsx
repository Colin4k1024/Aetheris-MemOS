import React from 'react';
import type { ActionType, ProColumns } from '@ant-design/pro-components';
import {
  PageContainer,
  ProTable,
  ModalForm,
  ProFormText,
  ProFormSelect,
} from '@ant-design/pro-components';
import { useRequest } from '@umijs/max';
import { Button, message, Popconfirm } from 'antd';
import { useRef, useState } from 'react';
import {
  listMemoryConfigs,
  createMemoryConfig,
  updateMemoryConfig,
  deleteMemoryConfig,
} from '@/services/memory/api';
import { MemoryConfigFormFields, StatusTag } from '@/components/MemorySystem';

// ── Shared modal form ────────────────────────────────────────────────────────

interface MemoryConfigFormModalProps {
  mode: 'create' | 'edit';
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialValues?: Record<string, any>;
  onFinish: (values: any) => Promise<boolean>;
}

const MemoryConfigFormModal: React.FC<MemoryConfigFormModalProps> = ({
  mode,
  open,
  onOpenChange,
  initialValues,
  onFinish,
}) => (
  <ModalForm
    title={mode === 'create' ? '创建记忆配置' : '编辑记忆配置'}
    open={open}
    onOpenChange={onOpenChange}
    initialValues={initialValues}
    onFinish={onFinish}
    width={800}
  >
    <ProFormText
      name="userId"
      label="用户ID"
      rules={[{ required: true, message: '请输入用户ID' }]}
    />
    <ProFormText
      name="agentId"
      label="智能体ID"
      rules={[{ required: true, message: '请输入智能体ID' }]}
    />
    <ProFormText
      name="configName"
      label="配置名称"
      rules={[{ required: true, message: '请输入配置名称' }]}
    />
    <ProFormSelect
      name="configType"
      label="配置类型"
      options={[
        { label: '默认', value: 'default' },
        { label: '自定义', value: 'custom' },
        { label: '优化', value: 'optimized' },
      ]}
      rules={[{ required: true, message: '请选择配置类型' }]}
    />
    <ProFormSelect
      name="status"
      label="状态"
      options={[
        { label: '激活', value: 'active' },
        { label: '未激活', value: 'inactive' },
        { label: '测试中', value: 'testing' },
      ]}
      rules={[{ required: true, message: '请选择状态' }]}
    />
    <MemoryConfigFormFields withDefaults={mode === 'create'} />
  </ModalForm>
);

// ── Page ──────────────────────────────────────────────────────────────────────

const MemoryManagement: React.FC = () => {
  const actionRef = useRef<ActionType | undefined>(undefined);
  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [editModalVisible, setEditModalVisible] = useState(false);
  const [currentRecord, setCurrentRecord] = useState<API.MemoryConfigRow | null>(null);

  const { run: deleteRun } = useRequest(deleteMemoryConfig, {
    manual: true,
    onSuccess: () => {
      message.success('删除成功');
      actionRef.current?.reload();
    },
    onError: () => message.error('删除失败，请重试'),
  });

  const columns: ProColumns<API.MemoryConfigRow>[] = [
    { title: '配置ID', dataIndex: 'config_id', width: 200, ellipsis: true, copyable: true },
    { title: '配置名称', dataIndex: 'config_name', width: 150 },
    { title: '用户ID', dataIndex: 'user_id', width: 150 },
    { title: '智能体ID', dataIndex: 'agent_id', width: 150 },
    {
      title: '配置类型',
      dataIndex: 'config_type',
      width: 120,
      valueEnum: {
        default: { text: '默认', status: 'Default' },
        custom: { text: '自定义', status: 'Processing' },
        optimized: { text: '优化', status: 'Success' },
      },
    },
    {
      title: '状态',
      dataIndex: 'status',
      width: 100,
      render: (_, record) => <StatusTag status={record.status} />,
    },
    {
      title: 'STM',
      dataIndex: 'stm_enabled',
      width: 80,
      render: (_, record) => (record.stm_enabled === 1 ? '启用' : '禁用'),
    },
    {
      title: 'LTM',
      dataIndex: 'ltm_enabled',
      width: 80,
      render: (_, record) => (record.ltm_enabled === 1 ? '启用' : '禁用'),
    },
    {
      title: 'KG',
      dataIndex: 'kg_enabled',
      width: 80,
      render: (_, record) => (record.kg_enabled === 1 ? '启用' : '禁用'),
    },
    {
      title: 'MM',
      dataIndex: 'mm_enabled',
      width: 80,
      render: (_, record) => (record.mm_enabled === 1 ? '启用' : '禁用'),
    },
    { title: '创建时间', dataIndex: 'created_at', width: 180, valueType: 'dateTime', sorter: true },
    {
      title: '操作',
      valueType: 'option',
      width: 200,
      render: (_, record) => [
        <Button
          key="edit"
          type="link"
          size="small"
          onClick={() => {
            setCurrentRecord(record);
            setEditModalVisible(true);
          }}
        >
          编辑
        </Button>,
        <Popconfirm
          key="delete"
          title="确定要删除这个配置吗？"
          onConfirm={() => deleteRun(record.config_id)}
        >
          <Button type="link" size="small" danger>
            删除
          </Button>
        </Popconfirm>,
      ],
    },
  ];

  const mapFormToCreate = (values: any): API.CreateMemoryConfigRequest => ({
    userId: values.userId,
    agentId: values.agentId,
    configName: values.configName,
    configType: values.configType,
    status: values.status,
    stmEnabled: values.stmEnabled ? 1 : 0,
    stmMaxLength: values.stmMaxLength || 4096,
    stmRetentionHours: values.stmRetentionHours || 24,
    ltmEnabled: values.ltmEnabled ? 1 : 0,
    ltmMaxEntries: values.ltmMaxEntries || 10000,
    ltmQualityThreshold: values.ltmQualityThreshold || 0.5,
    kgEnabled: values.kgEnabled ? 1 : 0,
    kgMaxEntities: values.kgMaxEntities || 1000,
    kgConfidenceThreshold: values.kgConfidenceThreshold || 0.7,
    mmEnabled: values.mmEnabled ? 1 : 0,
    mmMaxEntries: values.mmMaxEntries || 1000,
    mmModalityTypes: values.mmModalityTypes,
    maxResponseTimeMs: values.maxResponseTimeMs || 2000,
    maxMemoryUsageMb: values.maxMemoryUsageMb || 1024,
    maxCpuUsagePercent: values.maxCpuUsagePercent || 80,
  });

  const mapFormToUpdate = (values: any): API.UpdateMemoryConfigRequest => ({
    userId: values.userId,
    agentId: values.agentId,
    configName: values.configName,
    configType: values.configType,
    status: values.status,
    stmEnabled: values.stmEnabled ? 1 : 0,
    stmMaxLength: values.stmMaxLength,
    stmRetentionHours: values.stmRetentionHours,
    ltmEnabled: values.ltmEnabled ? 1 : 0,
    ltmMaxEntries: values.ltmMaxEntries,
    ltmQualityThreshold: values.ltmQualityThreshold,
    kgEnabled: values.kgEnabled ? 1 : 0,
    kgMaxEntities: values.kgMaxEntities,
    kgConfidenceThreshold: values.kgConfidenceThreshold,
    mmEnabled: values.mmEnabled ? 1 : 0,
    mmMaxEntries: values.mmMaxEntries,
    mmModalityTypes: values.mmModalityTypes,
    maxResponseTimeMs: values.maxResponseTimeMs,
    maxMemoryUsageMb: values.maxMemoryUsageMb,
    maxCpuUsagePercent: values.maxCpuUsagePercent,
  });

  return (
    <PageContainer>
      <ProTable<API.MemoryConfigRow>
        headerTitle="记忆配置列表"
        actionRef={actionRef}
        rowKey="config_id"
        search={{ labelWidth: 120 }}
        toolBarRender={() => [
          <Button type="primary" key="primary" onClick={() => setCreateModalVisible(true)}>
            新建配置
          </Button>,
        ]}
        request={async (params) => {
          const response = await listMemoryConfigs({
            page: params.current,
            pageSize: params.pageSize,
            userId: params.user_id,
            agentId: params.agent_id,
            status: params.status,
            configType: params.config_type,
          });
          return { data: response.data || [], success: true, total: response.total || 0 };
        }}
        columns={columns}
      />

      <MemoryConfigFormModal
        mode="create"
        open={createModalVisible}
        onOpenChange={setCreateModalVisible}
        onFinish={async (values) => {
          try {
            await createMemoryConfig(mapFormToCreate(values));
            message.success('创建成功');
            setCreateModalVisible(false);
            actionRef.current?.reload();
            return true;
          } catch {
            message.error('创建失败，请重试');
            return false;
          }
        }}
      />

      <MemoryConfigFormModal
        mode="edit"
        open={editModalVisible}
        onOpenChange={setEditModalVisible}
        initialValues={
          currentRecord
            ? {
                userId: currentRecord.user_id,
                agentId: currentRecord.agent_id,
                configName: currentRecord.config_name,
                configType: currentRecord.config_type,
                status: currentRecord.status,
                stmEnabled: currentRecord.stm_enabled === 1,
                stmMaxLength: currentRecord.stm_max_length,
                stmRetentionHours: currentRecord.stm_retention_hours,
                ltmEnabled: currentRecord.ltm_enabled === 1,
                ltmMaxEntries: currentRecord.ltm_max_entries,
                ltmQualityThreshold: currentRecord.ltm_quality_threshold,
                kgEnabled: currentRecord.kg_enabled === 1,
                kgMaxEntities: currentRecord.kg_max_entities,
                kgConfidenceThreshold: currentRecord.kg_confidence_threshold,
                mmEnabled: currentRecord.mm_enabled === 1,
                mmMaxEntries: currentRecord.mm_max_entries,
                mmModalityTypes: currentRecord.mm_modality_types,
                maxResponseTimeMs: currentRecord.max_response_time_ms,
                maxMemoryUsageMb: currentRecord.max_memory_usage_mb,
                maxCpuUsagePercent: currentRecord.max_cpu_usage_percent,
              }
            : undefined
        }
        onFinish={async (values) => {
          if (!currentRecord) return false;
          try {
            await updateMemoryConfig(currentRecord.config_id, mapFormToUpdate(values));
            message.success('更新成功');
            setEditModalVisible(false);
            setCurrentRecord(null);
            actionRef.current?.reload();
            return true;
          } catch {
            message.error('更新失败，请重试');
            return false;
          }
        }}
      />
    </PageContainer>
  );
};

export default MemoryManagement;
