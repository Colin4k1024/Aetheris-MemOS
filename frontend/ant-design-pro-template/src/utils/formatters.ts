/**
 * Shared formatting utilities.
 * Keeps chart label formatters and display helpers consistent across all pages.
 */

/** Format a 0-1 ratio as a percentage string, e.g. 0.856 → "85.60%" */
export const formatPercent = (val: number, decimals = 2): string =>
  `${(val * 100).toFixed(decimals)}%`;

/** Format a weight value to a fixed number of decimal places. */
export const formatWeight = (val: number, decimals = 2): string =>
  val.toFixed(decimals);

/**
 * Format a Unix timestamp (ms) or ISO string as HH:MM.
 * Used in chart x-axis label formatters.
 */
export const formatTime = (text: string | number): string => {
  const ts = typeof text === 'string' ? parseInt(text, 10) : text;
  const date = new Date(ts);
  const h = date.getHours().toString().padStart(2, '0');
  const m = date.getMinutes().toString().padStart(2, '0');
  return `${h}:${m}`;
};

/**
 * Format a Unix timestamp (ms) or ISO string as MM/DD HH:MM.
 * Used in weight-history chart labels.
 */
export const formatDateTime = (text: string | number): string => {
  try {
    const date = new Date(typeof text === 'string' ? text : text);
    const month = (date.getMonth() + 1).toString();
    const day = date.getDate().toString();
    const h = date.getHours().toString().padStart(2, '0');
    const m = date.getMinutes().toString().padStart(2, '0');
    return `${month}/${day} ${h}:${m}`;
  } catch {
    return String(text);
  }
};

/** Format a timestamp or ISO string as HH:MM:SS for resource-monitor charts. */
export const formatTimeWithSeconds = (text: string | number): string => {
  const ts = typeof text === 'string' ? parseInt(text, 10) : text;
  const date = new Date(ts);
  const h = date.getHours().toString().padStart(2, '0');
  const m = date.getMinutes().toString().padStart(2, '0');
  const s = date.getSeconds().toString().padStart(2, '0');
  return `${h}:${m}:${s}`;
};
