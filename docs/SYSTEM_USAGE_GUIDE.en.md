# Adaptive Memory System Usage Guide

## 1. System Overview

The Adaptive Memory System is an intelligent memory management system designed to provide efficient, adaptive memory management capabilities for agents. The system achieves adaptive optimization of the memory system by dynamically adjusting memory weights, optimizing resource allocation, and intelligently scheduling memory strategies, thereby improving agent performance and efficiency.

## 2. Architecture Design

### 2.1 Overall Architecture

The system uses a frontend-backend separated architecture:

- **Backend**: Built with Rust + Axum framework, providing RESTful API services
- **Frontend**: Built with React + Ant Design Pro, providing visual operation interface
- **Database**:
  - Primary Data: SQLite (extensible to PostgreSQL)
  - Vector Storage: Qdrant
  - Knowledge Graph: Neo4j (planned)

### 2.2 Core Components

1. **Adaptive Memory Scheduler**: Selects optimal memory configuration based on task context and resource constraints
2. **Task Characteristic Analyzer**: Analyzes task characteristics to determine memory requirements
3. **Performance Prediction Model**: Predicts performance for specific memory configurations
4. **Resource Monitor and Optimizer**: Monitors system resource usage and provides optimization suggestions
5. **Dynamic Weight Adjuster**: Dynamically adjusts weights for each memory layer
6. **Memory Storage Management**: Manages Short-Term Memory (STM) and Long-Term Memory (LTM)
7. **Memory Search Module**: Provides various memory search methods

## 3. Deployment Requirements

### 3.1 Hardware Requirements

- CPU: 4 cores or above
- Memory: 8GB or above
- Storage: 100GB or above
- Network: Stable network connection

### 3.2 Software Requirements

#### Backend

- Rust 1.89+
- Cargo
- SQLite 3.0+
- Qdrant 1.7+
- Neo4j 4.0+ (optional)

#### Frontend

- Node.js 16+
- npm or yarn
- Modern browser

## 4. Configuration Guide

### 4.1 Backend Configuration

The backend configuration file is located at `backend/config.toml`, with main configuration items:

```toml
# Server Configuration
listen_addr = "127.0.0.1:8008"

# Database Configuration
[db]
url = "file:./data/sqlx.sqlite"

# JWT Authentication Configuration (Use strong random key in production, do not use example values)
[jwt]
secret = "<your-jwt-secret>"
expiry = 3600

# Log Configuration
[log]
file_name = "app.log"
rolling = "daily"

# LLM Configuration
[llm]
base_url = "http://localhost:11434"
model = "llama3"
timeout_seconds = 30

# Embedding Model Configuration
[embedding]
base_url = "http://localhost:11434"
model = "nomic-embed-text"
dimension = 768
timeout_seconds = 30

# Qdrant Vector Database Configuration
[qdrant]
host = "localhost"
port = 6334
collection_name = "long_term_memory"
vector_dimension = 768
distance_type = "Euclid"

# Rerank Configuration
[rerank]
base_url = "http://localhost:11434"
model = "bge-reranker-base"
enabled = true
candidate_multiplier = 2
min_score_threshold = 0.3
timeout_seconds = 30

# Neo4j Graph Database Configuration
[neo4j]
host = "localhost"
port = 7687
username = "neo4j"
password = "<your-neo4j-password>"
database = "neo4j"
```

### 4.2 Frontend Configuration

The frontend configuration file is located at `frontend/ant-design-pro-template/config/config.ts`:

```ts
// API Request Configuration
export default {
  // Development Environment API Address
  dev: {
    baseURL: "http://127.0.0.1:8008",
  },
  // Production Environment API Address
  test: {
    baseURL: "https://api.example.com",
  },
  // Staging Environment API Address
  pre: {
    baseURL: "https://api.pre.example.com",
  },
};
```

## 5. Quick Start

### 5.1 Backend Startup

1. Navigate to the backend directory:

   ```bash
   cd backend
   ```

2. Start the development server:

   ```bash
   cargo run
   ```

3. Start the production server:
   ```bash
   cargo build --release
   ./target/release/backend
   ```

### 5.2 Frontend Startup

1. Navigate to the frontend directory:

   ```bash
   cd frontend/ant-design-pro-template
   ```

2. Install dependencies:

   ```bash
   npm install
   ```

3. Start the development server:

   ```bash
   npm run dev
   ```

4. Build for production:
   ```bash
   npm run build
   ```

## 6. Usage Workflow

### 6.1 Basic Usage Workflow

1. **Start Services**: Start backend and frontend services
2. **Access System**: Access the frontend in browser (default: http://localhost:8000)
3. **Login**: Login with default admin account
4. **Configure System**: Configure system parameters as needed
5. **Use API**: Use system features through frontend interface or direct API calls
6. **Monitor Performance**: Monitor system performance through frontend interface

### 6.2 API Call Workflow

1. **Get Authentication Token**: Obtain Bearer Token via login API
2. **Analyze Task Characteristics**: Call task characteristic analysis API to analyze task
3. **Select Memory Configuration**: Call adaptive memory selection API to get optimal configuration
4. **Execute Task**: Execute task with selected memory configuration
5. **Monitor Resources**: Call resource monitoring API to monitor system status
6. **Optimize Configuration**: Optimize memory configuration based on monitoring results

## 7. FAQ

### 7.1 Backend Startup Fails

- Check if dependencies are installed correctly
- Check if configuration file is correct
- Check if port is already in use

### 7.2 Frontend Cannot Connect to Backend

- Check if backend service is running
- Check if API address in frontend configuration is correct
- Check if network connection is normal

### 7.3 Qdrant Connection Fails

- Check if Qdrant service is running
- Check if Qdrant configuration is correct
- Check firewall settings

### 7.4 Memory Search Results Are Inaccurate

- Check if embedding model configuration is correct
- Check if vector dimensions match
- Try adjusting search parameters

## 8. Contact

For any questions or suggestions, please contact the system administrator or development team.

---

**Version**: 1.0.0  
**Last Updated**: 2025-12-30
