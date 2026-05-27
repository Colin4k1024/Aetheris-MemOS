import { createStyles } from 'antd-style';
import { SPACING } from './tokens';

const useDashboardStyles = createStyles(({ token }) => ({
  row: {
    marginBottom: SPACING.lg,
  },
  section: {
    marginBottom: SPACING.xl,
  },
  metricRow: {
    marginBottom: SPACING.lg,
  },
  chartRow: {
    marginBottom: SPACING.lg,
  },
  pageTitle: {
    fontSize: 20,
    fontWeight: 600,
    color: token.colorText,
    marginBottom: SPACING.lg,
  },
}));

export default useDashboardStyles;
