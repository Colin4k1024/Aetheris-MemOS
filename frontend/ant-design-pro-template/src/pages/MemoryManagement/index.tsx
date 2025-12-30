import type { ActionType, ProColumns } from '@ant-design/pro-components';
import {
  PageContainer,
  ProTable,
  ModalForm,
  ProFormText,
  ProFormSelect,
  ProFormDigit,
  ProFormSwitch,
  ProFormTextArea,
} from '@ant-design/pro-components';
import { useIntl, useRequest } from '@umijs/max';
import { Button, message, Popconfirm, Space } from 'antd';
import React, { useRef, useState } from 'react';
import {
  listMemoryConfigs,
  getMemoryConfig,
  createMemoryConfig,
  updateMemoryConfig,
  deleteMemoryConfig,
} from '@/services/memory/api';

const MemoryManagement: React.FC = () => {
  const actionRef = useRef<ActionType>();
  const [createModalVisible, setCreateModalVisible] = useState<boolean>(false);
  const [editModalVisible, setEditModalVisible] = useState<boolean>(false);
  const [currentRecord, setCurrentRecord] = useState<API.MemoryConfigRow | null>(null);
  const intl = useIntl();

  const { run: deleteRun } = useRequest(deleteMemoryConfig, {
    manual: true,
    onSuccess: () => {
      message.success('删除成功');
      actionRef.current?.reload();
    },
    onError: () => {
      message.error('删除失败，请重试');
    },
  });

  const columns: ProColumns<API.MemoryConfigRow>[] = [
    {
      title: '配置ID',
      dataIndex: 'config_id',
      width: 200,
      ellipsis: true,
      copyable: true,
    },
    {
      title: '配置名称',
      dataIndex: 'config_name',
      width: 150,
    },
    {
      title: '用户ID',
      dataIndex: 'user_id',
      width: 150,
    },
    {
      title: '智能体ID',
      dataIndex: 'agent_id',
      width: 150,
    },
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
      valueEnum: {
        active: { text: '激活', status: 'Success' },
        inactive: { text: '未激活', status: 'Default' },
        testing: { text: '测试中', status: 'Processing' },
      },
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
    {
      title: '创建时间',
      dataIndex: 'created_at',
      width: 180,
      valueType: 'dateTime',
      sorter: true,
    },
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
          onConfirm={() => {
            deleteRun(record.config_id);
          }}
        >
          <Button type="link" size="small" danger>
            删除
          </Button>
        </Popconfirm>,
      ],
    },
  ];

  return (
    <PageContainer>
      <ProTable<API.MemoryConfigRow>
        headerTitle="记忆配置列表"
        actionRef={actionRef}
        rowKey="config_id"
        search={{
          labelWidth: 120,
        }}
        toolBarRender={() => [
          <Button
            type="primary"
            key="primary"
            onClick={() => {
              setCreateModalVisible(true);
            }}
          >
            新建配置
          </Button>,
        ]}
        request={async (params, sort) => {
          const response = await listMemoryConfigs({
            page: params.current,
            pageSize: params.pageSize,
            userId: params.user_id,
            agentId: params.agent_id,
            status: params.status,
            configType: params.config_type,
          });
          return {
            data: response.data || [],
            success: true,
            total: response.total || 0,
          };
        }}
        columns={columns}
      />

      {/* 创建表单 */}
      <ModalForm
        title="创建记忆配置"
        open={createModalVisible}
        onOpenChange={setCreateModalVisible}
        onFinish={async (values) => {
          try {
            // 转换 Switch 值为 0/1
            const createData: API.CreateMemoryConfigRequest = {
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
            };
            await createMemoryConfig(createData);
            message.success('创建成功');
            setCreateModalVisible(false);
            actionRef.current?.reload();
            return true;
          } catch (error) {
            message.error('创建失败，请重试');
            return false;
          }
        }}
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
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>短期记忆 (STM)</h3>
          <ProFormSwitch name="stmEnabled" label="启用STM" />
          <ProFormDigit
            name="stmMaxLength"
            label="最大长度"
            min={1}
            initialValue={4096}
          />
          <ProFormDigit
            name="stmRetentionHours"
            label="保留时间(小时)"
            min={1}
            initialValue={24}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>长期记忆 (LTM)</h3>
          <ProFormSwitch name="ltmEnabled" label="启用LTM" />
          <ProFormDigit
            name="ltmMaxEntries"
            label="最大条目数"
            min={1}
            initialValue={10000}
          />
          <ProFormDigit
            name="ltmQualityThreshold"
            label="质量阈值"
            min={0}
            max={1}
            step={0.01}
            initialValue={0.5}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>知识图谱 (KG)</h3>
          <ProFormSwitch name="kgEnabled" label="启用KG" />
          <ProFormDigit
            name="kgMaxEntities"
            label="最大实体数"
            min={1}
            initialValue={1000}
          />
          <ProFormDigit
            name="kgConfidenceThreshold"
            label="置信度阈值"
            min={0}
            max={1}
            step={0.01}
            initialValue={0.7}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>多模态记忆 (MM)</h3>
          <ProFormSwitch name="mmEnabled" label="启用MM" />
          <ProFormDigit
            name="mmMaxEntries"
            label="最大条目数"
            min={1}
            initialValue={1000}
          />
          <ProFormTextArea name="mmModalityTypes" label="模态类型" />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>性能限制</h3>
          <ProFormDigit
            name="maxResponseTimeMs"
            label="最大响应时间(ms)"
            min={1}
            initialValue={2000}
          />
          <ProFormDigit
            name="maxMemoryUsageMb"
            label="最大内存使用(MB)"
            min={1}
            initialValue={1024}
          />
          <ProFormDigit
            name="maxCpuUsagePercent"
            label="最大CPU使用率(%)"
            min={1}
            max={100}
            initialValue={80}
          />
        </Space>
      </ModalForm>

      {/* 编辑表单 */}
      <ModalForm
        title="编辑记忆配置"
        open={editModalVisible}
        onOpenChange={setEditModalVisible}
        initialValues={currentRecord ? {
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
        } : undefined}
        onFinish={async (values) => {
          if (!currentRecord) return false;
          try {
            // 转换 Switch 值为 0/1
            const updateData: API.UpdateMemoryConfigRequest = {
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
            };
            await updateMemoryConfig(currentRecord.config_id, updateData);
            message.success('更新成功');
            setEditModalVisible(false);
            setCurrentRecord(null);
            actionRef.current?.reload();
            return true;
          } catch (error) {
            message.error('更新失败，请重试');
            return false;
          }
        }}
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
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>短期记忆 (STM)</h3>
          <ProFormSwitch name="stmEnabled" label="启用STM" />
          <ProFormDigit
            name="stmMaxLength"
            label="最大长度"
            min={1}
          />
          <ProFormDigit
            name="stmRetentionHours"
            label="保留时间(小时)"
            min={1}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>长期记忆 (LTM)</h3>
          <ProFormSwitch name="ltmEnabled" label="启用LTM" />
          <ProFormDigit
            name="ltmMaxEntries"
            label="最大条目数"
            min={1}
          />
          <ProFormDigit
            name="ltmQualityThreshold"
            label="质量阈值"
            min={0}
            max={1}
            step={0.01}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>知识图谱 (KG)</h3>
          <ProFormSwitch name="kgEnabled" label="启用KG" />
          <ProFormDigit
            name="kgMaxEntities"
            label="最大实体数"
            min={1}
          />
          <ProFormDigit
            name="kgConfidenceThreshold"
            label="置信度阈值"
            min={0}
            max={1}
            step={0.01}
          />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>多模态记忆 (MM)</h3>
          <ProFormSwitch name="mmEnabled" label="启用MM" />
          <ProFormDigit
            name="mmMaxEntries"
            label="最大条目数"
            min={1}
          />
          <ProFormTextArea name="mmModalityTypes" label="模态类型" />
        </Space>
        <Space direction="vertical" style={{ width: '100%' }}>
          <h3>性能限制</h3>
          <ProFormDigit
            name="maxResponseTimeMs"
            label="最大响应时间(ms)"
            min={1}
          />
          <ProFormDigit
            name="maxMemoryUsageMb"
            label="最大内存使用(MB)"
            min={1}
          />
          <ProFormDigit
            name="maxCpuUsagePercent"
            label="最大CPU使用率(%)"
            min={1}
            max={100}
          />
        </Space>
      </ModalForm>
    </PageContainer>
  );
};

export default MemoryManagement;

