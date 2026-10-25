// SPDX-License-Identifier: MIT OR Apache-2.0
// The install queue as the shell sees it: the whole list from `queue_watch`, kept current.
// Installs that finish are reported once (to refresh that game's library) and dismissed
// from the list a few seconds later (docs/08, "Downloads").

import { useEffect, useRef, useState } from "react";
import { api, type Job } from "./api";

/** How long an installed row stays on the Downloads page. */
export const CLEAR_DONE_AFTER_MS = 5000;

export function useQueue(onInstalled: (job: Job) => void): Job[] {
  const [jobs, setJobs] = useState<Job[]>([]);
  const seen = useRef(new Set<number>());
  const onInstalledRef = useRef(onInstalled);
  onInstalledRef.current = onInstalled;

  useEffect(() => {
    let live = true;
    api
      .queueWatch((list) => {
        if (live) setJobs(list);
      })
      .catch(() => {
        // Without a watcher the page stays empty; installs still run.
      });
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    for (const job of jobs) {
      if (job.state.type !== "done" || seen.current.has(job.id)) continue;
      seen.current.add(job.id);
      onInstalledRef.current(job);
      setTimeout(() => void api.queueDismiss(job.id).catch(() => {}), CLEAR_DONE_AFTER_MS);
    }
  }, [jobs]);

  return jobs;
}

/** Still in the queue or running (not finished, failed or cancelled). */
export function isActive(job: Job): boolean {
  return !["done", "failed", "cancelled"].includes(job.state.type);
}
