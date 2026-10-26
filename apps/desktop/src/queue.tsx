// SPDX-License-Identifier: MIT OR Apache-2.0
// The install queue as the shell sees it: the whole list from `queue_watch`, kept current.
// Installs that finish are reported once (to refresh that game's library) and dismissed
// from the list a few seconds later. The list lives in a context of
// its own: progress arrives several times a second while something downloads, and only what
// shows it (the sidebar's count, the Downloads page) should draw again, not the page the
// user is browsing.

import { createContext, type ReactNode, useContext, useEffect, useRef, useState } from "react";
import { api, type Job } from "./api";

/** How long an installed row stays on the Downloads page. */
export const CLEAR_DONE_AFTER_MS = 5000;

const JobsContext = createContext<Job[]>([]);

export function QueueProvider({ onInstalled, children }: { onInstalled: (job: Job) => void; children: ReactNode }) {
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

  return <JobsContext.Provider value={jobs}>{children}</JobsContext.Provider>;
}

/** Every job in the queue (the Downloads page, the sidebar's count). */
export function useJobs(): Job[] {
  return useContext(JobsContext);
}

/** Still in the queue, running or waiting for the user (not finished, failed or cancelled). */
export function isActive(job: Job): boolean {
  return !["done", "failed", "cancelled"].includes(job.state.type);
}
