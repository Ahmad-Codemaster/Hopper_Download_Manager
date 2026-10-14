import type { Job, JobEvent } from "./api";

export type Category = "all" | "downloading" | "queued" | "completed" | "failed" | "paused";

export function applyJobEvent(jobs: Job[], event: JobEvent): Job[] {
  return jobs.map((job) =>
    job.id === event.job_id
      ? { ...job, state: event.state, completed_bytes: event.completed_bytes, total_bytes: event.total_bytes ?? job.total_bytes, speed_bytes_per_sec: event.speed_bytes_per_sec }
      : job
  );
}

export function matchesCategory(job: Job, category: Category): boolean {
  if (category === "all") return true;
  if (category === "downloading") return job.state === "Active";
  return job.state.toLowerCase() === category;
}

export function filterJobs(jobs: Job[], query: string, category: Category): Job[] {
  const normalized = query.trim().toLowerCase();
  return jobs.filter((job) => matchesCategory(job, category) && (!normalized || `${job.destination} ${job.source}`.toLowerCase().includes(normalized)));
}

export function getJobCounts(jobs: Job[]): Record<Category, number> {
  return {
    all: jobs.length,
    downloading: jobs.filter((job) => job.state === "Active").length,
    queued: jobs.filter((job) => job.state === "Queued").length,
    completed: jobs.filter((job) => job.state === "Completed").length,
    failed: jobs.filter((job) => job.state === "Failed").length,
    paused: jobs.filter((job) => job.state === "Paused").length,
  };
}
