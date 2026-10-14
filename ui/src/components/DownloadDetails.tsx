import { ArrowLeft, Pause, Play, RefreshCw, X } from "lucide-react";
import type { Job } from "../lib/api";
import { formatBytes, formatEta, formatSpeed, sourceHost } from "../lib/format";

type Action = "pause_job" | "resume_job" | "cancel_job" | "retry_job";
export function DownloadDetails({ job, onBack, onAction, pendingAction = false }: { job: Job; onBack: () => void; onAction: (action: Action) => void; pendingAction?: boolean }) {
  const percent = job.total_bytes ? Math.min(100, (job.completed_bytes / job.total_bytes) * 100) : 0;
  return <section className="details">
    <button className="back-button" onClick={onBack}><ArrowLeft size={17} /> All downloads</button>
    <div className="details-header"><div><span className="eyebrow">Download details</span><h2>{job.destination}</h2><p>{sourceHost(job.source)} · {job.destination}</p></div><span className={`status status-${job.state.toLowerCase()}`}>{job.state}</span></div>
    <div className="details-progress"><div className="progress-large"><span style={{ width: `${percent}%` }} /></div><strong>{job.total_bytes ? `${percent.toFixed(0)}%` : "Progress unavailable"}</strong></div>
    <div className="metrics"><div><span>Downloaded</span><strong>{formatBytes(job.completed_bytes)}</strong></div><div><span>Total size</span><strong>{formatBytes(job.total_bytes)}</strong></div><div><span>Speed</span><strong>{formatSpeed(job.speed_bytes_per_sec)}</strong></div><div><span>Remaining</span><strong>{formatEta(job.completed_bytes, job.total_bytes, job.speed_bytes_per_sec) ?? "Unknown"}</strong></div></div>
    <div className="detail-actions">{job.state === "Active" && <button disabled={pendingAction} onClick={() => onAction("pause_job")}><Pause size={16} /> Pause</button>}{(job.state === "Paused" || job.state === "Queued") && <button disabled={pendingAction} onClick={() => onAction("resume_job")}><Play size={16} /> Resume</button>}{job.state === "Failed" && <button disabled={pendingAction} onClick={() => onAction("retry_job")}><RefreshCw size={16} /> Retry</button>}{["Active", "Paused", "Queued"].includes(job.state) && <button disabled={pendingAction} className="danger-button" onClick={() => onAction("cancel_job")}><X size={16} /> Cancel</button>}</div>
    <details className="technical"><summary>Technical details</summary><dl><dt>Source</dt><dd>{sourceHost(job.source)}</dd><dt>Destination</dt><dd>{job.destination}</dd><dt>HTTP diagnostics</dt><dd>Unknown</dd><dt>Connection activity</dt><dd>Segment details are not exposed by the transfer engine.</dd></dl></details>
  </section>;
}
