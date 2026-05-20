/**
 * @name umi 的路由配置
 * @description 只支持 path,component,routes,redirect,wrappers,name,icon 的配置
 * @doc https://umijs.org/docs/guides/routes
 */
export default [
  {
    path: '/home',
    layout: false,
    hideInMenu: true,
    component: './Home',
  },
  {
    path: '/user',
    layout: false,
    routes: [
      {
        name: 'login',
        path: '/user/login',
        component: './user/login',
      },
    ],
  },
  {
    path: '/documentation',
    layout: false,
    hideInMenu: true,
    component: './Documentation',
  },
  // ── 监控 ──────────────────────────────────────
  {
    name: 'monitor-group',
    icon: 'monitor',
    path: '/monitor',
    routes: [
      {
        path: '/dashboard',
        name: 'dashboard',
        icon: 'dashboard',
        component: './Dashboard',
      },
      {
        path: '/performance',
        name: 'performance',
        icon: 'lineChart',
        component: './Performance',
      },
      {
        path: '/resource-monitor',
        name: 'resource-monitor',
        icon: 'barChart',
        component: './ResourceMonitor',
      },
    ],
  },
  // ── 记忆管理 ──────────────────────────────────
  {
    name: 'memory-group',
    icon: 'database',
    path: '/memory',
    routes: [
      {
        path: '/memory-management',
        name: 'memory-management',
        icon: 'database',
        component: './MemoryManagement',
      },
      {
        path: '/memory-details',
        name: 'memory-details',
        icon: 'folderOpen',
        component: './MemoryDetails',
      },
      {
        path: '/memory-config',
        name: 'memory-config',
        icon: 'setting',
        component: './MemoryConfig',
      },
    ],
  },
  // ── 分析追踪 ──────────────────────────────────
  {
    name: 'analysis-group',
    icon: 'fileSearch',
    path: '/analysis',
    routes: [
      {
        path: '/task-analysis',
        name: 'task-analysis',
        icon: 'fileSearch',
        component: './TaskAnalysis',
      },
      {
        path: '/weight-history',
        name: 'weight-history',
        icon: 'history',
        component: './WeightHistory',
      },
      {
        path: '/memory-decision-trace',
        name: 'memory-decision-trace',
        icon: 'apartment',
        component: './MemoryDecisionTrace',
      },
    ],
  },
  {
    path: '/',
    redirect: '/home',
  },
  {
    path: '*',
    layout: false,
    component: './404',
  },
];
