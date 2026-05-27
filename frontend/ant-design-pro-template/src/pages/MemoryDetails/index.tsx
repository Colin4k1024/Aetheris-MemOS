import {
  ApiOutlined,
  DatabaseOutlined,
  MessageOutlined,
  PictureOutlined,
} from '@ant-design/icons';
import { PageContainer } from '@ant-design/pro-components';
import { Col, Row, Table, Tabs, Tag } from 'antd';
import React, { useEffect, useState } from 'react';
import { ChartCard, MetricCard } from '@/components/MemorySystem';
import { listEntities } from '@/services/memory/knowledgeGraphApi';
import { listMm } from '@/services/memory/multimodalApi';
import { listLtmEntries, listSessions } from '@/services/memory/storageApi';

const MemoryDetails: React.FC = () => {
  const [activeTab, setActiveTab] = useState('stm');
  const [stmLoading, setStmLoading] = useState(false);
  const [ltmLoading, setLtmLoading] = useState(false);
  const [kgLoading, setKgLoading] = useState(false);
  const [mmLoading, setMmLoading] = useState(false);

  const [sessions, setSessions] = useState<API.SessionInfo[]>([]);
  const [ltmEntries, setLtmEntries] = useState<API.KnowledgeEntry[]>([]);
  const [kgEntities, setKgEntities] = useState<API.EntityInfo[]>([]);
  const [mmEntries, setMmEntries] = useState<API.MMEntryInfo[]>([]);

  const [stmStats, setStmStats] = useState({ sessions: 0, messages: 0 });
  const [ltmStats, setLtmStats] = useState({ entries: 0 });
  const [kgStats, setKgStats] = useState({ entities: 0 });
  const [mmStats, setMmStats] = useState({ entries: 0 });

  // Pre-fetch all four layer stats on mount so the metric cards at the top
  // show real numbers before the user manually switches tabs.
  useEffect(() => {
    loadSessions();
    loadLtmEntries();
    loadKgEntities();
    loadMmEntries();
  }, []);

  useEffect(() => {
    if (activeTab === 'stm') loadSessions();
    else if (activeTab === 'ltm') loadLtmEntries();
    else if (activeTab === 'kg') loadKgEntities();
    else if (activeTab === 'mm') loadMmEntries();
  }, [activeTab]);

  const loadSessions = async () => {
    setStmLoading(true);
    try {
      const response = await listSessions({ limit: 100 });
      setSessions(response.sessions || []);
      const totalMessages = (response.sessions || []).reduce(
        (sum, s) => sum + (s.message_count || 0),
        0,
      );
      setStmStats({ sessions: response.total || 0, messages: totalMessages });
    } catch {
      // silently handle
    } finally {
      setStmLoading(false);
    }
  };

  const loadLtmEntries = async () => {
    setLtmLoading(true);
    try {
      const response = await listLtmEntries({ limit: 100 });
      setLtmEntries(response.entries || []);
      setLtmStats({ entries: response.total || 0 });
    } catch {
      // silently handle
    } finally {
      setLtmLoading(false);
    }
  };

  const loadKgEntities = async () => {
    setKgLoading(true);
    try {
      const response = await listEntities({ limit: 100 });
      setKgEntities(response.entities || []);
      setKgStats({ entities: response.total || 0 });
    } catch {
      // silently handle
    } finally {
      setKgLoading(false);
    }
  };

  const loadMmEntries = async () => {
    setMmLoading(true);
    try {
      const response = await listMm({ limit: 100 });
      setMmEntries(response.entries || []);
      setMmStats({ entries: response.total || 0 });
    } catch {
      // silently handle
    } finally {
      setMmLoading(false);
    }
  };

  const stmColumns = [
    {
      title: '会话ID',
      dataIndex: 'session_id',
      key: 'session_id',
      width: 200,
      ellipsis: true,
    },
    { title: '用户ID', dataIndex: 'user_id', key: 'user_id', width: 150 },
    { title: '智能体ID', dataIndex: 'agent_id', key: 'agent_id', width: 150 },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      width: 100,
      render: (status: string) => (
        <Tag color={status === 'active' ? 'green' : 'default'}>
          {status === 'active' ? '活跃' : status}
        </Tag>
      ),
    },
    {
      title: '消息数',
      dataIndex: 'message_count',
      key: 'message_count',
      width: 100,
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
    },
    {
      title: '最后访问',
      dataIndex: 'last_accessed_at',
      key: 'last_accessed_at',
      width: 180,
    },
  ];

  const ltmColumns = [
    {
      title: '条目ID',
      dataIndex: 'entry_id',
      key: 'entry_id',
      width: 200,
      ellipsis: true,
    },
    {
      title: '标题',
      dataIndex: 'title',
      key: 'title',
      width: 200,
      ellipsis: true,
    },
    {
      title: '内容类型',
      dataIndex: 'content_type',
      key: 'content_type',
      width: 120,
      render: (type: string) => <Tag>{type}</Tag>,
    },
    {
      title: '来源类型',
      dataIndex: 'source_type',
      key: 'source_type',
      width: 100,
    },
    {
      title: '质量分数',
      dataIndex: 'quality_score',
      key: 'quality_score',
      width: 100,
      render: (score: number) => score?.toFixed(2) || '-',
    },
    {
      title: '类别',
      dataIndex: 'category',
      key: 'category',
      width: 100,
      render: (cat: string) => cat || '-',
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
    },
  ];

  const kgColumns = [
    {
      title: '实体ID',
      dataIndex: 'entity_id',
      key: 'entity_id',
      width: 200,
      ellipsis: true,
    },
    {
      title: '实体名称',
      dataIndex: 'entity_name',
      key: 'entity_name',
      width: 150,
    },
    {
      title: '实体类型',
      dataIndex: 'entity_type',
      key: 'entity_type',
      width: 120,
      render: (type: string) => <Tag color="blue">{type}</Tag>,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
    },
  ];

  const mmColumns = [
    {
      title: '条目ID',
      dataIndex: 'entry_id',
      key: 'entry_id',
      width: 200,
      ellipsis: true,
    },
    {
      title: '标题',
      dataIndex: 'title',
      key: 'title',
      width: 200,
      ellipsis: true,
    },
    {
      title: '模态类型',
      dataIndex: 'modality_type',
      key: 'modality_type',
      width: 120,
      render: (type: string) => {
        const colorMap: Record<string, string> = {
          image: 'green',
          audio: 'orange',
          video: 'purple',
          text: 'blue',
        };
        return <Tag color={colorMap[type] || 'default'}>{type}</Tag>;
      },
    },
    {
      title: '来源ID',
      dataIndex: 'source_id',
      key: 'source_id',
      width: 150,
      ellipsis: true,
    },
    {
      title: '会话ID',
      dataIndex: 'session_id',
      key: 'session_id',
      width: 150,
      ellipsis: true,
      render: (id: string) => id || '-',
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (desc: string) => desc || '-',
    },
  ];

  const tabItems = [
    {
      key: 'stm',
      label: (
        <span>
          <MessageOutlined /> 短期记忆 (STM)
        </span>
      ),
      children: (
        <ChartCard title="" loading={stmLoading}>
          <Table
            dataSource={sessions}
            columns={stmColumns}
            rowKey="session_id"
            pagination={{ pageSize: 10 }}
            size="small"
          />
        </ChartCard>
      ),
    },
    {
      key: 'ltm',
      label: (
        <span>
          <DatabaseOutlined /> 长期记忆 (LTM)
        </span>
      ),
      children: (
        <ChartCard title="" loading={ltmLoading}>
          <Table
            dataSource={ltmEntries}
            columns={ltmColumns}
            rowKey="entry_id"
            pagination={{ pageSize: 10 }}
            size="small"
          />
        </ChartCard>
      ),
    },
    {
      key: 'kg',
      label: (
        <span>
          <ApiOutlined /> 知识图谱 (KG)
        </span>
      ),
      children: (
        <ChartCard title="" loading={kgLoading}>
          <Table
            dataSource={kgEntities}
            columns={kgColumns}
            rowKey="entity_id"
            pagination={{ pageSize: 10 }}
            size="small"
          />
        </ChartCard>
      ),
    },
    {
      key: 'mm',
      label: (
        <span>
          <PictureOutlined /> 多模态记忆 (MM)
        </span>
      ),
      children: (
        <ChartCard title="" loading={mmLoading}>
          <Table
            dataSource={mmEntries}
            columns={mmColumns}
            rowKey="entry_id"
            pagination={{ pageSize: 10 }}
            size="small"
          />
        </ChartCard>
      ),
    },
  ];

  return (
    <PageContainer>
      <Row gutter={[24, 24]} style={{ marginBottom: 24 }}>
        <Col span={6}>
          <MetricCard
            title="短期记忆 (STM)"
            value={stmStats.sessions}
            unit={`/ ${stmStats.messages} 消息`}
            icon={<MessageOutlined />}
            variant="primary"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="长期记忆 (LTM)"
            value={ltmStats.entries}
            icon={<DatabaseOutlined />}
            variant="success"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="知识图谱 (KG)"
            value={kgStats.entities}
            icon={<ApiOutlined />}
            variant="info"
          />
        </Col>
        <Col span={6}>
          <MetricCard
            title="多模态记忆 (MM)"
            value={mmStats.entries}
            icon={<PictureOutlined />}
            variant="warning"
          />
        </Col>
      </Row>

      <Tabs activeKey={activeTab} onChange={setActiveTab} items={tabItems} />
    </PageContainer>
  );
};

export default MemoryDetails;
