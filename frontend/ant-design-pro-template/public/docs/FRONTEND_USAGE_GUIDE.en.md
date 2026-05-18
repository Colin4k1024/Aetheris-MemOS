# Adaptive Memory System Frontend Usage Guide

## 1. Frontend Architecture

### 1.1 Tech Stack

- **Framework**: React 18
- **UI Component Library**: Ant Design Pro
- **Development Framework**: UmiJS
- **State Management**: React Hooks
- **API Requests**: Axios
- **Styling**: Less + CSS Modules

### 1.2 Project Structure

The frontend project is located in the `frontend/ant-design-pro-template` directory, with the following structure:

```
frontend/ant-design-pro-template/
├── config/            # Project configuration
├── mock/              # Mock data
├── public/            # Static assets
├── src/
│   ├── components/    # Shared components
│   ├── locales/       # Internationalization configuration
│   ├── pages/         # Page components
│   ├── services/      # API services
│   ├── app.tsx        # Application entry
│   └── global.less    # Global styles
├── tests/             # Test files
└── types/             # TypeScript type definitions
```

## 2. Page Navigation

### 2.1 Navigation Structure

The system uses left sidebar + top navigation bar layout:

- **Top Navigation Bar**: Displays current user info, system notifications, language switch, etc.
- **Left Navigation Bar**: Contains main functional modules, as follows:

| Navigation Item | Path | Function |
|-----------------|------|----------|
| Dashboard | /dashboard | System overview, key metrics display |
| Task Analysis | /task-analysis | Task characteristic analysis, memory strategy recommendation |
| Memory Config | /memory-config | Memory configuration management, weight adjustment |
| Performance | /performance | System performance monitoring, prediction model evaluation |
| Resource Monitor | /resource-monitor | System resource usage monitoring |
| Weight History | /weight-history | Memory weight adjustment history |
| Memory Management | /memory-management | Memory storage and retrieval management |

### 2.2 Login Page

**Path**: `/user/login`

**Function**: User login to system, obtain authentication credentials

**How to Use**:
1. Enter username and password
2. Click "Login" button
3. After successful login, automatically redirect to dashboard page

**Default Account** (for local/demo only, please change in production):
- Username: See account created during deployment
- Password: See password set during deployment

## 3. Page Function Descriptions

### 3.1 Dashboard

**Path**: `/dashboard`

**Function**: Display system overview and key metrics, provide quick access to features

**Main Content**:

1. **System Status Card**: Display system health status, response time, success rate, etc.
2. **Resource Usage Chart**: Real-time display of CPU, memory, storage resource usage
3. **Memory Configuration Overview**: Display current memory configuration and weight distribution
4. **Recent Task Analysis**: Show recent task characteristic analysis results
5. **Quick Actions**: Provide quick access buttons to common features

**How to Use**:
1. View system status card to understand overall system operation
2. Observe resource usage chart to monitor resource consumption
3. Click memory configuration overview to enter memory configuration page
4. Click recent task analysis to view detailed task characteristic analysis
5. Use quick action buttons for quick access to common features

### 3.2 Task Analysis

**Path**: `/task-analysis`

**Function**: Analyze task characteristics, recommend optimal memory strategy

**Main Content**:

1. **Task Characteristic Input**: Provide input interface for task content, modality requirements, etc.
2. **Characteristic Analysis Results**: Display task complexity, reasoning depth and other analysis results

**How to Use**:
1. Enter task content in the input field
2. Select task modality requirements (text, image, audio, video)
3. Click "Analyze" button to analyze task characteristics
4. View analysis results including complexity score, recommended memory strategy, etc.

### 3.3 Memory Configuration

**Path**: `/memory-config`

**Function**: Manage memory configurations, adjust memory weights

**Main Content**:

1. **Configuration List**: Display all memory configurations
2. **Configuration Editor**: Create and modify memory configurations
3. **Weight Adjustment**: Adjust weights for each memory layer (STM, LTM, KG, MM)

**How to Use**:
1. View existing memory configurations in the list
2. Click "Add Configuration" to create new configuration
3. Adjust memory layer weights using sliders
4. Save configuration after adjustments

### 3.4 Performance Monitoring

**Path**: `/performance`

**Function**: Monitor system performance, evaluate prediction models

**Main Content**:

1. **Performance Metrics**: Display efficiency score, coherence score, response time, etc.
2. **Performance Trends**: Show historical performance data trends
3. **Model Evaluation**: Evaluate prediction model accuracy

**How to Use**:
1. View real-time performance metrics
2. Analyze performance trends over different time periods
3. Evaluate prediction model performance

### 3.5 Resource Monitoring

**Path**: `/resource-monitor`

**Function**: Monitor system resource usage in real-time

**Main Content**:

1. **Resource Overview**: Display CPU, memory, storage usage
2. **Resource Alerts**: Show resource usage alerts and warnings
3. **Resource History**: Historical resource usage data

**How to Use**:
1. View current resource usage status
2. Set resource alert thresholds
3. View historical resource usage data

### 3.6 Weight History

**Path**: `/weight-history`

**Function**: View memory weight adjustment history

**Main Content**:

1. **History List**: Display all weight adjustment records
2. **Adjustment Details**: Show details of each adjustment including before/after weights, reasons
3. **Trend Analysis**: Analyze weight adjustment trends

**How to Use**:
1. Browse weight adjustment history
2. Filter by time range, task type, etc.
3. Analyze adjustment patterns and trends

### 3.7 Memory Management

**Path**: `/memory-management`

**Function**: Manage memory storage and retrieval

**Main Content**:

1. **Memory Search**: Search short-term and long-term memories
2. **Memory Storage**: Store new memories manually
3. **Memory Transfer**: Transfer short-term memories to long-term storage

**How to Use**:
1. Search for existing memories
2. Manually store important information as memories
3. Transfer session memories to long-term memory

## 4. Common Operations

### 4.1 Adjust Memory Weights

1. Navigate to Memory Configuration page
2. Select a configuration or create new one
3. Adjust STM, LTM, KG, MM weights using sliders
4. Click "Save" to apply changes

### 4.2 Analyze Task

1. Navigate to Task Analysis page
2. Enter task content
3. Select task modality
4. Click "Analyze" button
5. View recommended memory strategy

### 4.3 Monitor Resources

1. Navigate to Resource Monitoring page
2. View real-time resource usage
3. Set alert thresholds if needed

## 5. Configuration

### 5.1 API Configuration

Frontend API configuration is located in `config/config.ts`:

```ts
export default {
  dev: {
    baseURL: 'http://127.0.0.1:8008',
  },
  test: {
    baseURL: 'https://api.example.com',
  },
  pre: {
    baseURL: 'https://api.pre.example.com',
  },
};
```

### 5.2 Internationalization

The system supports English and Chinese. Switch language via the language selector in the top navigation bar.

## 6. Troubleshooting

### 6.1 Page Does Not Load

- Check if backend service is running
- Check if API configuration is correct
- Check browser console for error messages

### 6.2 API Request Fails

- Verify network connection
- Check if user is logged in
- Check API endpoint permissions

### 6.3 Chart Not Displaying

- Check if data is loading correctly
- Check browser console for JavaScript errors
- Verify chart configuration

---

**Version**: 1.0.0  
**Last Updated**: 2025-12-30
