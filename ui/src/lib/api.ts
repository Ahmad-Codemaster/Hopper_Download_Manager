import { invoke } from "@tauri-apps/api/core";

export type Job = {
  id: { 0: string };
  destination: string;
  state: string;
  completed_bytes: number;
};

export async function listJobs(): Promise<Job[]> {
  return invoke<Job[]>("list_jobs");
}

export async function createAndStart(source: string, destination: string): Promise<string> {
  const id = await invoke<string>("create_job", { source, destination });
  await invoke("start_job", { jobId: id });
  return id;
}

export async function controlJob(command: "pause_job" | "resume_job" | "cancel_job" | "retry_job", jobId: string): Promise<void> {
  await invoke(command, { jobId });
}
