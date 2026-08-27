import {
  AuditOutlined,
  ClockCircleOutlined,
  SafetyOutlined,
  UndoOutlined,
} from '@ant-design/icons';
import { PageContainer } from '@ant-design/pro-components';
import {
  Button,
  Descriptions,
  Drawer,
  message,
  Popconfirm,
  Space,
  Table,
  Tabs,
  Tag,
  Typography,
} from 'antd';
import React, { useCallback, useEffect, useState } from 'react';
import {
  archiveBelief,
  confirmBelief,
  denyBelief,
  getBeliefTrace,
  getGovernanceStats,
  listBeliefs,
  listCandidates,
  rollbackBelief,
  type GovernanceAuditRow,
  type GovernanceBelief,
  type GovernanceCandidate,
  type GovernanceEvidence,
  type GovernanceStats,
} from '@/services/memory/governanceApi';

const { Text } = Typography;

const statusColor: Record<string, string> = {
  active: 'green',
  needs_confirm: 'orange',
  quarantined: 'red',
  stale: 'default',
  superseded: 'blue',
  archived: 'default',
  rejected: 'red',
  pending: 'orange',
};

const MemoryGovernance: React.FC = () => {
  const [stats, setStats] = useState<GovernanceStats>({ active_beliefs: 0, pending_confirm: 0, quarantined: 0 });
  const [beliefs, setBeliefs] = useState<GovernanceBelief[]>([]);
  const [includeHistory, setIncludeHistory] = useState(false);
  const [candidates, setCandidates] = useState<GovernanceCandidate[]>([]);
  const [queue, setQueue] = useState<'pending' | 'quarantined'>('pending');
  const [loading, setLoading] = useState(false);
  const [traceOpen, setTraceOpen] = useState(false);
  const [trace, setTrace] = useState<{
    belief: GovernanceBelief;
    evidence: GovernanceEvidence[];
    audit: GovernanceAuditRow[];
  } | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [s, b] = await Promise.all([
        getGovernanceStats(),
        listBeliefs({ include_history: includeHistory, limit: 200 }),
      ]);
      setStats(s);
      setBeliefs(b.beliefs);
    } catch (e) {
      message.error(`加载失败: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [includeHistory]);

  const refreshCandidates = useCallback(async (status: 'pending' | 'quarantined') => {
    try {
      const c = await listCandidates({ status, limit: 200 });
      setCandidates(c.candidates);
    } catch {
      // Non-admin callers are rejected by the backend: show an empty queue
      // rather than an error toast on every tab switch.
      setCandidates([]);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    refreshCandidates(queue);
  }, [queue, refreshCandidates]);

  const act = async (fn: () => Promise<{ ok: boolean; detail?: string | null }>, okText: string) => {
    try {
      const r = await fn();
      if (r.ok) message.success(okText);
      else message.warning(r.detail ?? '无变更');
      refresh();
      refreshCandidates(queue);
    } catch (e) {
      message.error(`操作失败: ${e}`);
    }
  };

  const openTrace = async (id: string) => {
    try {
      const t = await getBeliefTrace(id);
      setTrace(t);
      setTraceOpen(true);
    } catch (e) {
      message.error(`追踪失败: ${e}`);
    }
  };

  const beliefColumns = [
    { title: '主体', dataIndex: 'subject', ellipsis: true, width: 220 },
    { title: '谓词', dataIndex: 'predicate', width: 120 },
    { title: '客体', dataIndex: 'object', ellipsis: true },
    {
      title: '状态',
      dataIndex: 'status',
      width: 120,
      render: (s: string) => <Tag color={statusColor[s] || 'default'}>{s}</Tag>,
    },
    { title: '来源', dataIndex: 'source', width: 130 },
    { title: '信任', dataIndex: 'trust', width: 80, render: (t: number) => t.toFixed(2) },
    { title: '生效自', dataIndex: 'valid_from', width: 180, render: (v: string) => v?.slice(0, 19) },
    { title: '失效于', dataIndex: 'valid_to', width: 180, render: (v: string | null) => v?.slice(0, 19) ?? '—' },
    {
      title: '操作',
      width: 260,
      render: (_: unknown, b: GovernanceBelief) => (
        <Space size="small">
          <Button size="small" icon={<AuditOutlined />} onClick={() => openTrace(b.id)}>
            追踪
          </Button>
          {b.status === 'needs_confirm' && (
            <>
              <Popconfirm title="确认该信念为真？" onConfirm={() => act(() => confirmBelief(b.id), '已确认')}>
                <Button size="small" type="primary">
                  确认
                </Button>
              </Popconfirm>
              <Popconfirm title="否决该信念？" onConfirm={() => act(() => denyBelief(b.id), '已否决')}>
                <Button size="small" danger>
                  否决
                </Button>
              </Popconfirm>
            </>
          )}
          {b.status === 'active' && (
            <>
              <Popconfirm title="回滚到前一版本？" onConfirm={() => act(() => rollbackBelief(b.id), '已回滚')}>
                <Button size="small" icon={<UndoOutlined />}>
                  回滚
                </Button>
              </Popconfirm>
              <Popconfirm title="归档该信念？" onConfirm={() => act(() => archiveBelief(b.id), '已归档')}>
                <Button size="small">归档</Button>
              </Popconfirm>
            </>
          )}
        </Space>
      ),
    },
  ];

  const candidateColumns = [
    { title: '主体', dataIndex: 'subject', ellipsis: true, width: 220 },
    { title: '谓词', dataIndex: 'predicate', width: 120 },
    { title: '内容', dataIndex: 'object', ellipsis: true },
    { title: '来源', dataIndex: 'source', width: 130 },
    { title: '状态', dataIndex: 'status', width: 110, render: (s: string) => <Tag color={statusColor[s]}>{s}</Tag> },
    { title: '原因', dataIndex: 'rejection_reason', ellipsis: true },
  ];

  return (
    <PageContainer
      header={{ title: '记忆治理', subTitle: '信念生命周期 · 来源与信任 · 冲突与隔离队列（#124 Epic）' }}
    >
      <Tabs
        items={[
          {
            key: 'beliefs',
            label: '信念',
            children: (
              <>
                <Descriptions size="small" bordered column={3} style={{ marginBottom: 16 }}>
                  <Descriptions.Item label={<>现行信念</>}>
                    {stats.active_beliefs}
                  </Descriptions.Item>
                  <Descriptions.Item label={<>待确认</>}>
                    {stats.pending_confirm}
                  </Descriptions.Item>
                  <Descriptions.Item label={<>隔离区</>}>
                    {stats.quarantined}
                  </Descriptions.Item>
                </Descriptions>
                <Space style={{ marginBottom: 12 }}>
                  <Button
                    size="small"
                    type={includeHistory ? 'primary' : 'default'}
                    onClick={() => setIncludeHistory(!includeHistory)}
                  >
                    {includeHistory ? '含历史版本' : '仅现行边'}
                  </Button>
                  <Text type="secondary">
                    时间线、来源与信任随版本保留；回滚把当前边换回其前驱。
                  </Text>
                </Space>
                <Table
                  rowKey="id"
                  size="small"
                  loading={loading}
                  columns={beliefColumns}
                  dataSource={beliefs}
                  pagination={{ pageSize: 20, showSizeChanger: false }}
                />
              </>
            ),
          },
          {
            key: 'queues',
            label: '队列',
            children: (
              <>
                <Tabs
                  activeKey={queue}
                  onChange={(k) => setQueue(k as 'pending' | 'quarantined')}
                  items={[
                    { key: 'pending', label: `待确认 (${stats.pending_confirm})` },
                    { key: 'quarantined', label: `隔离区 (${stats.quarantined})` },
                  ]}
                />
                <Table
                  rowKey="id"
                  size="small"
                  columns={candidateColumns}
                  dataSource={candidates}
                  pagination={{ pageSize: 20, showSizeChanger: false }}
                />
              </>
            ),
          },
        ]}
      />

      <Drawer
        title="信念追踪（belief · event · provenance）"
        width={720}
        open={traceOpen}
        onClose={() => setTraceOpen(false)}
      >
        {trace && (
          <>
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item label="SPO">
                {trace.belief.subject} · {trace.belief.predicate} · {trace.belief.object}
              </Descriptions.Item>
              <Descriptions.Item label="状态/来源/信任">
                <Tag color={statusColor[trace.belief.status]}>{trace.belief.status}</Tag>{' '}
                {trace.belief.source} / {trace.belief.trust.toFixed(2)} / risk {trace.belief.risk}
              </Descriptions.Item>
              <Descriptions.Item label="有效窗口">
                {trace.belief.valid_from.slice(0, 19)} → {trace.belief.valid_to?.slice(0, 19) ?? '现在'}
              </Descriptions.Item>
            </Descriptions>
            <Typography.Title level={5}>证据（不可变事件）</Typography.Title>
            <Table
              rowKey="id"
              size="small"
              pagination={false}
              dataSource={trace.evidence}
              columns={[
                { title: '事件', dataIndex: 'event_id', ellipsis: true },
                { title: '类型', dataIndex: 'kind', width: 90 },
                { title: '内容哈希', dataIndex: 'content_hash', ellipsis: true, render: (h: string) => <Text code>{h.slice(0, 16)}…</Text> },
              ]}
            />
            <Typography.Title level={5}>审计链</Typography.Title>
            <Table
              rowKey="event_id"
              size="small"
              pagination={false}
              dataSource={trace.audit}
              columns={[
                { title: '时间', dataIndex: 'created_at', width: 180, render: (v: string | null) => v?.slice(0, 19) ?? '—' },
                { title: '事件', dataIndex: 'event_type', width: 200 },
                { title: '操作者', dataIndex: 'actor_id', width: 140 },
              ]}
            />
          </>
        )}
      </Drawer>
    </PageContainer>
  );
};

export default MemoryGovernance;
