import {
  ApartmentOutlined,
  ArrowRightOutlined,
  ClusterOutlined,
  DashboardOutlined,
  GithubOutlined,
  LoginOutlined,
  PictureOutlined,
  ReadOutlined,
  RocketOutlined,
  ShareAltOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { history } from '@umijs/max';
import { Button, Card, Col, Row, Space, Typography } from 'antd';
import React from 'react';
import useStyles from './style';

const { Title, Paragraph } = Typography;

const features = [
  {
    icon: <ThunderboltOutlined />,
    title: 'Adaptive Memory Selection',
    titleZh: '自适应记忆选择',
    desc: '基于任务复杂度、时间范围和推理深度，智能选择最优记忆配置方案',
  },
  {
    icon: <ClusterOutlined />,
    title: 'Multi-Layer Architecture',
    titleZh: '多层记忆架构',
    desc: '短期记忆(STM)、长期记忆(LTM)、知识图谱(KG)、多模态(MM)四层协同',
  },
  {
    icon: <ApartmentOutlined />,
    title: 'Decision Tracing',
    titleZh: '决策链路追踪',
    desc: '完整的决策证据图谱，支持 SHA-256 哈希链验证，确保可审计可回溯',
  },
  {
    icon: <ShareAltOutlined />,
    title: 'Knowledge Graph',
    titleZh: '知识图谱引擎',
    desc: '实体关系建模与图遍历检索，支持 GraphRAG 混合搜索',
  },
  {
    icon: <PictureOutlined />,
    title: 'Multi-Modal Memory',
    titleZh: '多模态记忆',
    desc: '文本、图像、音频等多模态数据的统一存储与语义检索',
  },
  {
    icon: <RocketOutlined />,
    title: 'Performance Optimization',
    titleZh: '性能优化引擎',
    desc: '动态权重调整、资源监控、成本效益分析，持续优化系统表现',
  },
];

const memoryLayers = [
  { name: 'STM', label: '短期记忆', color: '#e6f7ff', border: '#91d5ff' },
  { name: 'LTM', label: '长期记忆', color: '#f6ffed', border: '#b7eb8f' },
  { name: 'KG', label: '知识图谱', color: '#fff7e6', border: '#ffd591' },
  { name: 'MM', label: '多模态', color: '#f9f0ff', border: '#d3adf7' },
];

const HomePage: React.FC = () => {
  const { styles } = useStyles();

  return (
    <div className={styles.container}>
      {/* Header */}
      <header className={styles.header}>
        <div className={styles.logo}>
          <ThunderboltOutlined />
          Aetheris-MemOS
        </div>
        <nav className={styles.navLinks}>
          <a
            className={styles.navLink}
            onClick={() => history.push('/dashboard')}
          >
            <DashboardOutlined /> Dashboard
          </a>
          <a
            className={styles.navLink}
            onClick={() => history.push('/documentation')}
          >
            <ReadOutlined /> Documentation
          </a>
          <a
            className={styles.navLink}
            href="https://github.com"
            target="_blank"
            rel="noopener noreferrer"
          >
            <GithubOutlined /> GitHub
          </a>
          <Button
            type="primary"
            size="small"
            icon={<LoginOutlined />}
            onClick={() => history.push('/user/login')}
          >
            Login
          </Button>
        </nav>
      </header>

      {/* Hero Section */}
      <section className={styles.hero}>
        <h1 className={styles.heroTitle}>Aetheris-MemOS</h1>
        <p className={styles.heroSubtitle}>
          Adaptive Memory Operating System for AI Agents
        </p>
        <p className={styles.heroDescription}>
          为 AI Agent 和 LLM
          工作负载打造的自适应记忆管理系统。通过多层记忆架构、
          智能调度策略和完整的决策追踪，让 AI 具备更强的记忆能力和推理表现。
        </p>
        <div className={styles.heroButtons}>
          <Button
            type="primary"
            size="large"
            icon={<ReadOutlined />}
            onClick={() => history.push('/documentation')}
          >
            查看文档
          </Button>
          <Button
            size="large"
            icon={<DashboardOutlined />}
            onClick={() => history.push('/dashboard')}
          >
            进入控制台
          </Button>
        </div>
      </section>

      {/* Features Section */}
      <section className={styles.features}>
        <h2 className={styles.featuresTitle}>核心能力</h2>
        <Row gutter={[24, 24]}>
          {features.map((feature) => (
            <Col xs={24} sm={12} lg={8} key={feature.title}>
              <Card className={styles.featureCard} bordered={false}>
                <div className={styles.featureIcon}>{feature.icon}</div>
                <div className={styles.featureTitle}>{feature.titleZh}</div>
                <div className={styles.featureDesc}>{feature.desc}</div>
              </Card>
            </Col>
          ))}
        </Row>
      </section>

      {/* Architecture Section */}
      <section className={styles.architecture}>
        <h2 className={styles.architectureTitle}>系统架构</h2>
        <div className={styles.architectureDiagram}>
          <div className={styles.archFlow}>
            <div className={styles.archNode}>Client / Agent</div>
            <span className={styles.archArrow}>→</span>
            <div className={styles.archNode}>API Gateway</div>
            <span className={styles.archArrow}>→</span>
            <div className={styles.archNode}>Scheduler</div>
            <span className={styles.archArrow}>→</span>
            <div className={styles.archNode}>Memory Layers</div>
          </div>
          <div className={styles.archLayers}>
            {memoryLayers.map((layer) => (
              <div
                key={layer.name}
                className={styles.archLayer}
                style={{
                  background: layer.color,
                  border: `1px solid ${layer.border}`,
                }}
              >
                <div style={{ fontWeight: 700, marginBottom: 4 }}>
                  {layer.name}
                </div>
                <div style={{ fontSize: 12, opacity: 0.8 }}>{layer.label}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className={styles.footer}>
        <Space direction="vertical" size={8}>
          <div>Aetheris-MemOS — Adaptive Memory Operating System</div>
          <div>Built with Rust (Axum) + React (Ant Design Pro)</div>
        </Space>
      </footer>
    </div>
  );
};

export default HomePage;
