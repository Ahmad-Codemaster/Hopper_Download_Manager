import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Job = {
  id: string;
  source: string;
  destination: string;
  state: "Queued" | "Active" | "Paused" | "Failed" | "Completed" | "Canceled";
  completed_bytes: number;
  total_bytes: number | null;
  speed_bytes_per_sec?: number;
};

export type JobEvent = {
  job_id: string;
  state: "Queued" | "Active" | "Paused" | "Failed" | "Completed" | "Canceled";
  completed_bytes: number;
  total_bytes: number | null;
  speed_bytes_per_sec: number;
  retry_count: number;
};

export async function listJobs(): Promise<Job[]> {
  return invoke<Job[]>("list_jobs");
}

export async function createAndStart(source: string, destination: string): Promise<string> {
  const id = await invoke<string>("create_job", { source, destination });
  await invoke("start_job", { jobId: id });
  return id;
}

export async function controlJob(
  command: "start_job" | "pause_job" | "resume_job" | "cancel_job" | "retry_job",
  jobId: string
): Promise<void> {
  await invoke(command, { jobId });
}

export async function refreshSource(jobId: string, newSource: string): Promise<void> {
  await invoke("refresh_source", { jobId, newSource });
}

export async function onProgress(callback: (event: JobEvent) => void): Promise<UnlistenFn> {
  return listen<JobEvent>("download_progress", (e) => callback(e.payload));
}

export type ZipEntryPreview = {
  name: string;
  uncompressed_size: number;
  compressed_size: number;
  is_dir: boolean;
};

export async function setSpeedLimit(limitBytesPerSec: number): Promise<void> {
  await invoke("set_speed_limit", { limitBytesPerSec });
}

export async function getSpeedLimit(): Promise<number> {
  return invoke<number>("get_speed_limit");
}

export async function setMaxConcurrent(maxConcurrent: number): Promise<void> {
  await invoke("set_max_concurrent", { maxConcurrent });
}

export async function getMaxConcurrent(): Promise<number> {
  return invoke<number>("get_max_concurrent");
}

export async function previewZipArchive(url: string, totalBytes: number): Promise<ZipEntryPreview[]> {
  return invoke<ZipEntryPreview[]>("preview_zip_archive", { url, totalBytes });
}

