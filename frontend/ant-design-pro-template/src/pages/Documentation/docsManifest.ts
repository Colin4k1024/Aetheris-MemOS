export interface DocItem {
  id: string;
  title: string;
  titleZh: string;
  path: string;
  pathZh?: string;
}

export interface DocCategory {
  key: string;
  title: string;
  titleZh: string;
  icon: string;
  children: DocItem[];
}

export const docsManifest: DocCategory[] = [
  {
    key: 'getting-started',
    title: 'Getting Started',
    titleZh: '快速开始',
    icon: 'rocket',
    children: [
      {
        id: 'readme',
        title: 'Overview',
        titleZh: '项目概览',
        path: 'README.md',
      },
      {
        id: 'system-usage',
        title: 'System Usage Guide',
        titleZh: '系统使用指南',
        path: 'SYSTEM_USAGE_GUIDE.en.md',
        pathZh: 'SYSTEM_USAGE_GUIDE.md',
      },
      {
        id: 'frontend-usage',
        title: 'Frontend Usage Guide',
        titleZh: '前端使用指南',
        path: 'FRONTEND_USAGE_GUIDE.en.md',
        pathZh: 'FRONTEND_USAGE_GUIDE.md',
      },
    ],
  },
  {
    key: 'architecture',
    title: 'Architecture',
    titleZh: '架构设计',
    icon: 'cluster',
    children: [
      {
        id: 'architecture',
        title: 'Architecture',
        titleZh: '系统架构',
        path: 'ARCHITECTURE.md',
        pathZh: 'ARCHITECTURE.zh.md',
      },
      {
        id: 'architecture-deep-dive',
        title: 'Architecture Deep Dive',
        titleZh: '架构深度解析',
        path: 'ARCHITECTURE_DEEP_DIVE.md',
      },
      {
        id: 'why-axum',
        title: 'Why Axum',
        titleZh: '为什么选择 Axum',
        path: 'why-axum.md',
        pathZh: 'why-axum.zh.md',
      },
    ],
  },
  {
    key: 'api',
    title: 'API Reference',
    titleZh: 'API 参考',
    icon: 'api',
    children: [
      {
        id: 'api-endpoints',
        title: 'API Endpoints',
        titleZh: 'API 端点列表',
        path: 'API_ENDPOINTS.md',
      },
      {
        id: 'api-examples',
        title: 'API Examples',
        titleZh: 'API 调用示例',
        path: 'API_EXAMPLES.md',
      },
      {
        id: 'api-usage-guide',
        title: 'API Usage Guide',
        titleZh: 'API 使用指南',
        path: 'API_USAGE_GUIDE.en.md',
        pathZh: 'API_USAGE_GUIDE.md',
      },
      {
        id: 'memory-api-examples',
        title: 'Memory API Examples',
        titleZh: '记忆 API 示例',
        path: 'MEMORY_API_EXAMPLES.md',
      },
      {
        id: 'api-specification',
        title: 'API Specification',
        titleZh: 'API 规格说明',
        path: 'adaptive_memory_api_specification.md',
        pathZh: 'adaptive_memory_api_specification.zh.md',
      },
    ],
  },
  {
    key: 'algorithm',
    title: 'Algorithm Design',
    titleZh: '算法设计',
    icon: 'experiment',
    children: [
      {
        id: 'algorithm-design',
        title: 'Algorithm Design',
        titleZh: '自适应算法设计',
        path: 'adaptive_memory_algorithm_design.en.md',
        pathZh: 'adaptive_memory_algorithm_design.md',
      },
      {
        id: 'algorithm-visualization',
        title: 'Algorithm Visualization',
        titleZh: '算法可视化',
        path: 'adaptive_memory_algorithm_visualization.md',
        pathZh: 'adaptive_memory_algorithm_visualization.zh.md',
      },
      {
        id: 'evidence-graph',
        title: 'Evidence Graph',
        titleZh: '证据图谱',
        path: 'evidence_graph.md',
      },
    ],
  },
  {
    key: 'integration',
    title: 'Integration',
    titleZh: '集成指南',
    icon: 'link',
    children: [
      {
        id: 'integration-guide',
        title: 'Integration Guide',
        titleZh: '集成指南',
        path: 'INTEGRATION_GUIDE.md',
      },
      {
        id: 'integration-cookbook',
        title: 'Integration Cookbook',
        titleZh: '集成食谱',
        path: 'INTEGRATION_COOKBOOK.md',
      },
      {
        id: 'extension-guide',
        title: 'Extension Guide',
        titleZh: '扩展开发指南',
        path: 'EXTENSION_GUIDE.md',
        pathZh: 'EXTENSION_GUIDE.zh.md',
      },
    ],
  },
  {
    key: 'project',
    title: 'Project',
    titleZh: '项目规划',
    icon: 'project',
    children: [
      {
        id: 'roadmap',
        title: 'Roadmap',
        titleZh: '路线图',
        path: 'ROADMAP.md',
        pathZh: 'ROADMAP.zh.md',
      },
      {
        id: 'use-cases',
        title: 'Use Cases',
        titleZh: '使用场景',
        path: 'USE_CASES.md',
        pathZh: 'USE_CASES.zh.md',
      },
      {
        id: 'open-core',
        title: 'Open Core Boundary',
        titleZh: '开源核心边界',
        path: 'OPEN_CORE_BOUNDARY.md',
      },
    ],
  },
];

export function findDocById(id: string): DocItem | undefined {
  for (const category of docsManifest) {
    const found = category.children.find((doc) => doc.id === id);
    if (found) return found;
  }
  return undefined;
}

export function getDefaultDocId(): string {
  return docsManifest[0]?.children[0]?.id ?? 'readme';
}
