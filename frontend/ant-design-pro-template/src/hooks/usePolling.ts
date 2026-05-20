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

  // Always use the latest version of fn without re-creating the interval.
  useEffect(() => {
    fnRef.current = fn;
  }, [fn]);

  useEffect(() => {
    if (!enabled) return undefined;

    fnRef.current();
    const id = setInterval(() => {
      fnRef.current();
    }, interval);

    return () => clearInterval(id);
  }, [enabled, interval]);
};

export default usePolling;
