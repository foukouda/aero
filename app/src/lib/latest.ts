// "Latest wins" scheduling for interactive requests (angle-of-attack
// slider): at most one request is in flight, and while it runs only the most
// recent argument is kept, so the interface never queues stale work.

export function latestRunner<T>(task: (arg: T) => Promise<void>): (arg: T) => Promise<void> {
  let running = false;
  let pending: { arg: T } | null = null;
  let idle: Promise<void> = Promise.resolve();

  async function drain(): Promise<void> {
    while (pending) {
      const { arg } = pending;
      pending = null;
      try {
        await task(arg);
      } catch {
        // The task reports its own errors; keep serving later requests.
      }
    }
    running = false;
  }

  return (arg: T) => {
    pending = { arg };
    if (!running) {
      running = true;
      idle = drain();
    }
    return idle;
  };
}
