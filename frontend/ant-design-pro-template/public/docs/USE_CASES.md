# Use Cases

Why fork or integrate this project: typical scenarios and audiences.

---

## LLM Agent memory management

- **Problem:** Agents need different memory budgets and layers per turn (e.g. short context for simple QA, LTM + KG for complex reasoning).
- **Use this system to:** Call the adaptive memory API with task context and constraints; get a recommended memory config (primary/secondary layers, weights). Use it to configure context window, retrieval, and KG usage per request.
- **Audience:** LLM app and agent framework authors who want a single “memory policy” service instead of ad-hoc rules.

---

## Multi-modal task orchestration

- **Problem:** Tasks mix text, image, and other modalities; over-enabling multimodal memory wastes cost, under-enabling hurts quality.
- **Use this system to:** Send modality requirements and preferences; receive a config that enables or down-weights multimodal memory (MM) and aligns STM/LTM/KG with the detected modality mix.
- **Audience:** Multi-modal RAG and agent pipelines.

---

## Cost-aware inference routing

- **Problem:** Need to stay within latency and resource caps while maximizing quality (efficiency/coherence).
- **Use this system to:** Pass resource constraints (max memory, CPU, response time, storage) and optional preferences (e.g. prioritize efficiency); get a memory configuration and predicted resource requirements. Use the result to route or throttle upstream (e.g. which models or retrievers to call).
- **Audience:** Platform and inference-routing teams.

---

## Learning and extension

- **Use this system as:** A reference implementation of adaptive memory (rule-based today, agent-ready design). Fork to add custom strategies, new analyzers, or LLM-backed decision steps. The frontend is designed to visualize both rule-based and (future) LLM-driven memory agents.
