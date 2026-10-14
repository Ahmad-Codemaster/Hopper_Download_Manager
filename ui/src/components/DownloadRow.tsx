import { Archive, File, FileAudio, FileImage, FileText, FileVideo, Pause, Play, RefreshCw, X } from "lucide-react";
import type { Job } from "../lib/api";
import { fileKind, formatBytes, formatEta, formatSpeed, sourceHost } from "../lib/format";

type Action = "pause_job" | "resume_job" | "cancel_job" | "retry_job";
type Props = { job: Job; onSelect: () => void; onAction: (action: Action) => void; pendingAction?: boolean; onPreview?: () => void };

const icons = { video: FileVideo, audio: FileAudio, archive: Archive, image: FileImage, document: FileText, file: File };

export function DownloadRow({ job, onSelect, onAction, pendingAction = false, onPreview }: Props) {
  const kind = fileKind(job.destination);
  const Icon = icons[kind];
  const percent = job.total_bytes ? Math.min(100, (job.completed_bytes / job.total_bytes) * 100) : 0;
  const eta = formatEta(job.completed_bytes, job.total_bytes, job.speed_bytes_per_sec);
  return (
    <article className="download-row" onClick={onSelect} tabIndex={0} onKeyDown={(event) => event.key === "Enter" && onSelect()}>
      <div className="file-icon"><Icon size={21} /></div>
      <div className="download-main">
        <div className="download-heading"><strong title={job.destination}>{job.destination}</strong><span className={`status status-${job.state.toLowerCase()}`}>{job.state}</span></div>
        <span className="source">{sourceHost(job.source)}</span>
        <div className="progress-track"><span style={{ width: `${percent}%` }} /></div>
        <div className="download-meta"><span>{formatBytes(job.completed_bytes)} / {formatBytes(job.total_bytes)}{job.total_bytes ? ` · ${percent.toFixed(0)}%` : ""}</span><span>{job.state === "Queued" ? "Waiting in queue" : formatSpeed(job.speed_bytes_per_sec)}{eta ? ` · ${eta}` : ""}</span></div>
      </div>
      <div className="row-actions">
        {onPreview && <button className="icon-button" title="Preview archive" aria-label="Preview archive" onClick={(event) => { event.stopPropagation(); onPreview(); }}><Archive size={16} /></button>}
        {job.state === "Active" && <button disabled={pendingAction} className="icon-button" title="Pause" aria-label="Pause" onClick={(event) => { event.stopPropagation(); onAction("pause_job"); }}><Pause size={16} /></button>}
        {(job.state === "Paused" || job.state === "Queued") && <button disabled={pendingAction} className="icon-button" title="Resume" aria-label="Resume" onClick={(event) => { event.stopPropagation(); onAction("resume_job"); }}><Play size={16} /></button>}
        {job.state === "Failed" && <button disabled={pendingAction} className="icon-button" title="Retry" aria-label="Retry" onClick={(event) => { event.stopPropagation(); onAction("retry_job"); }}><RefreshCw size={16} /></button>}
        {(job.state === "Active" || job.state === "Paused" || job.state === "Queued") && <button disabled={pendingAction} className="icon-button danger" title="Cancel" aria-label="Cancel" onClick={(event) => { event.stopPropagation(); onAction("cancel_job"); }}><X size={16} /></button>}
      </div>
    </article>
  );
}
