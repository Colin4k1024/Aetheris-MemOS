import { useEffect, useRef } from 'react';

export interface UsePollingOptions {
  /** Whether polling is active. Defaults to true. */
  enabled?: boolean;
  /** Polling interval in milliseconds. */
  interval: number;
}

/**
 * Executes `fn` immediately and then on every `interval` milliseconds.
 * Stops when `enabled` becomes false or the component unmounts.
 */
const usePolling = (fn: () => void, options: UsePollingOptions): void => {
  const { enabled = true, interval } = options;
  const fnRef = useRef(fn);
  // Tracks whether the initial immediate execution has already happened for the
  // current "enabled: true" activation. Prevents duplicate calls when only
  // `interval` changes while polling is already running.
  const hasRunRef = useRef(false);

  // Always use the latest version of fn without re-creating the interval.
  useEffect(() => {
    fnRef.current = fn;
  }, [fn]);

  useEffect(() => {
    if (!enabled) {
      // Reset so the next enable → fires immediately again.
      hasRunRef.current = false;
      return undefined;
    }

    if (!hasRunRef.current) {
      fnRef.current();
      hasRunRef.current = true;
    }

    const id = setInterval(() => {
      fnRef.current();
    }, interval);

    return () => clearInterval(id);
  }, [enabled, interval]);
};

export default usePolling;
