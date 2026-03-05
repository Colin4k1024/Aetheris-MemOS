/**
 * Common constants for forms and selects
 */

// ==================== Task Types ====================
export const TASK_TYPES = [
  { label: 'Query', value: 'query' },
  { label: 'Generation', value: 'generation' },
  { label: 'Analysis', value: 'analysis' },
  { label: 'Classification', value: 'classification' },
  { label: 'Translation', value: 'translation' },
  { label: 'Summarization', value: 'summarization' },
  { label: 'Extraction', value: 'extraction' },
  { label: 'Conversation', value: 'conversation' },
];

// ==================== Modalities ====================
export const MODALITIES = [
  { label: 'Text', value: 'text' },
  { label: 'Image', value: 'image' },
  { label: 'Audio', value: 'audio' },
  { label: 'Video', value: 'video' },
];

// ==================== Temporal Scopes ====================
export const TEMPORAL_SCOPES = [
  { label: 'Short', value: 'short' },
  { label: 'Medium', value: 'medium' },
  { label: 'Long', value: 'long' },
  { label: 'Indefinite', value: 'indefinite' },
];

// ==================== Reasoning Depths ====================
export const REASONING_DEPTHS = [
  { label: 'Shallow', value: 'shallow' },
  { label: 'Medium', value: 'medium' },
  { label: 'Deep', value: 'deep' },
];

// ==================== Memory Layers ====================
export const MEMORY_LAYERS = [
  { label: 'STM (Short-Term)', value: 'stm' },
  { label: 'LTM (Long-Term)', value: 'ltm' },
  { label: 'KG (Knowledge Graph)', value: 'kg' },
  { label: 'MM (Multimodal)', value: 'mm' },
];

// ==================== Config Status ====================
export const CONFIG_STATUS = [
  { label: 'Active', value: 'active', color: 'success' },
  { label: 'Inactive', value: 'inactive', color: 'default' },
  { label: 'Testing', value: 'testing', color: 'processing' },
  { label: 'Deprecated', value: 'deprecated', color: 'error' },
];

// ==================== Default Values ====================
export const DEFAULTS = {
  COMPLEXITY: 0.5,
  CONTEXT_DEPENDENCY: 0.5,
  MAX_MEMORY_MB: 1024,
  MAX_CPU_PERCENT: 80,
  MAX_RESPONSE_TIME_MS: 2000,
  STORAGE_QUOTA_PERCENT: 90,
  TOP_K: 10,
  PAGE_SIZE: 20,
};

export default {
  TASK_TYPES,
  MODALITIES,
  TEMPORAL_SCOPES,
  REASONING_DEPTHS,
  MEMORY_LAYERS,
  CONFIG_STATUS,
  DEFAULTS,
};
