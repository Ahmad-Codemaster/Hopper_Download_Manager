import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";
import {
  createAndStart,
  listJobs,
  controlJob,
  refreshSource,
  onProgress,
  setSpeedLimit,
  getSpeedLimit,
  setMaxConcurrent,
  getMaxConcurrent,
  previewZipArchive,
  type Job,
  type ZipEntryPreview,
} from "./lib/api";

function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || isNaN(bytes)) return "Unknown";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatSpeed(bytesPerSec: number | undefined): string {
  if (!bytesPerSec || bytesPerSec === 0) return "--";
  return `${formatBytes(bytesPerSec)}/s`;
}

function App() {
  const [url, setUrl] = useState("");
  const [jobs, setJobs] = useState<Job[]>([]);
  const [filter, setFilter] = useState<"All" | "Active" | "Paused" | "Completed">("All");
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [newUrl, setNewUrl] = useState("");
  const [speedLimit, setSpeedLimitState] = useState<number>(0);
  const [maxConcurrent, setMaxConcurrentState] = useState<number>(3);
  const [previewFiles, setPreviewFiles] = useState<ZipEntryPreview[] | null>(null);
  const [previewArchiveName, setPreviewArchiveName] = useState<string>("");
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);

  const reloadJobs = () => {
    listJobs()
      .then(setJobs)
      .catch((error) => console.error("Could not load downloads", error));
  };

  useEffect(() => {
    reloadJobs();
    getSpeedLimit().then(setSpeedLimitState).catch(() => {});
    getMaxConcurrent().then(setMaxConcurrentState).catch(() => {});

    let unlistenFn: (() => void) | undefined;
    onProgress((event) => {
      setJobs((currentJobs) =>
        currentJobs.map((job) => {
          if (job.id === event.job_id) {
            return {
              ...job,
              state: event.state,
              completed_bytes: event.completed_bytes,
              total_bytes: event.total_bytes ?? job.total_bytes,
              speed_bytes_per_sec: event.speed_bytes_per_sec,
            };
          }
          return job;
        })
      );
    }).then((unlisten) => {
      unlistenFn = unlisten;
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, []);

  const add = async () => {
    try {
      const parsed = new URL(url);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("HTTP(S) only");
      const filename = parsed.pathname.split("/").filter(Boolean).pop() || "download";
      const id = await createAndStart(parsed.toString(), filename);
      setJobs((current) => [
        ...current,
        {
          id,
          source: parsed.toString(),
          destination: filename,
          state: "Active",
          completed_bytes: 0,
          total_bytes: null,
          speed_bytes_per_sec: 0,
        },
      ]);
      setUrl("");
    } catch (error) {
      console.error(error);
      alert("Enter a valid HTTP or HTTPS URL.");
    }
  };

  const handleAction = async (
    command: "start_job" | "pause_job" | "resume_job" | "cancel_job" | "retry_job",
    jobId: string
  ) => {
    try {
      await controlJob(command, jobId);
      reloadJobs();
    } catch (e) {
      console.error(e);
    }
  };

  const submitRefresh = async (jobId: string) => {
    try {
      new URL(newUrl); // Validate
      await refreshSource(jobId, newUrl);
      setRefreshingId(null);
      setNewUrl("");
      await controlJob("start_job", jobId);
      reloadJobs();
    } catch (e) {
      alert("Please enter a valid HTTP(S) URL for refreshing.");
    }
  };

  const handleSpeedLimitChange = async (val: number) => {
    setSpeedLimitState(val);
    await setSpeedLimit(val);
  };

  const handleMaxConcurrentChange = async (val: number) => {
    setMaxConcurrentState(val);
    await setMaxConcurrent(val);
  };

  const handlePreviewArchive = async (job: Job) => {
    if (!job.total_bytes) return;
    setIsLoadingPreview(true);
    setPreviewArchiveName(job.destination);
    try {
      const files = await previewZipArchive(job.source, job.total_bytes);
      setPreviewFiles(files);
    } catch (err) {
      alert(`Could not preview archive: ${err}`);
    } finally {
      setIsLoadingPreview(false);
    }
  };

  const filteredJobs = jobs.filter((job) => {
    if (filter === "Active") return job.state === "Active" || job.state === "Queued";
    if (filter === "Paused") return job.state === "Paused";
    if (filter === "Completed") return job.state === "Completed";
    return true;
  });

  return (
    <main>
      <header>
        <div className="header-left">
          <div className="logo-badge">IDM</div>
          <div>
            <h1>Hopper</h1>
            <span>High-Speed Download Accelerator</span>
          </div>
        </div>

        <div className="header-controls">
          <div className="control-group">
            <label>⚡ Speed Limit:</label>
            <select
              value={speedLimit}
              onChange={(e) => handleSpeedLimitChange(Number(e.target.value))}
            >
              <option value={0}>Unlimited</option>
              <option value={1048576}>1 MB/s</option>
              <option value={2097152}>2 MB/s</option>
              <option value={5242880}>5 MB/s</option>
              <option value={10485760}>10 MB/s</option>
              <option value={20971520}>20 MB/s</option>
            </select>
          </div>

          <div className="control-group">
            <label>🚦 Max Active:</label>
            <select
              value={maxConcurrent}
              onChange={(e) => handleMaxConcurrentChange(Number(e.target.value))}
            >
              <option value={1}>1 at a time</option>
              <option value={2}>2 at a time</option>
              <option value={3}>3 at a time</option>
              <option value={5}>5 at a time</option>
              <option value={0}>Unlimited</option>
            </select>
          </div>
        </div>
      </header>

      <section className="add">
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="Paste direct download URL (games, videos, files)..."
        />
        <button className="primary-btn" onClick={add}>
          + Add Download
        </button>
      </section>

      <nav>
        <button className={filter === "All" ? "active" : ""} onClick={() => setFilter("All")}>
          All ({jobs.length})
        </button>
        <button
          className={filter === "Active" ? "active" : ""}
          onClick={() => setFilter("Active")}
        >
          Downloading (
          {jobs.filter((j) => j.state === "Active" || j.state === "Queued").length})
        </button>
        <button
          className={filter === "Paused" ? "active" : ""}
          onClick={() => setFilter("Paused")}
        >
          Paused ({jobs.filter((j) => j.state === "Paused").length})
        </button>
        <button
          className={filter === "Completed" ? "active" : ""}
          onClick={() => setFilter("Completed")}
        >
          Completed ({jobs.filter((j) => j.state === "Completed").length})
        </button>
      </nav>

      <section className="list">
        {filteredJobs.length === 0 ? (
          <p className="empty">No downloads found in this view.</p>
        ) : (
          filteredJobs.map((job) => {
            const percent =
              job.total_bytes && job.total_bytes > 0
                ? Math.min(100, (job.completed_bytes / job.total_bytes) * 100).toFixed(1)
                : null;

            const isZip = job.destination.toLowerCase().endsWith(".zip");

            return (
              <article key={job.id} className={`job-card ${job.state.toLowerCase()}`}>
                <div className="job-header">
                  <div>
                    <strong className="filename">{job.destination}</strong>
                    <span className={`badge ${job.state.toLowerCase()}`}>{job.state}</span>
                  </div>
                  <div className="actions">
                    {isZip && job.total_bytes && (
                      <button
                        className="action-btn preview"
                        onClick={() => handlePreviewArchive(job)}
                        disabled={isLoadingPreview}
                      >
                        📦 {isLoadingPreview ? "Inspecting..." : "Preview ZIP"}
                      </button>
                    )}
                    {job.state === "Active" && (
                      <button
                        className="action-btn"
                        onClick={() => handleAction("pause_job", job.id)}
                      >
                        ⏸ Pause
                      </button>
                    )}
                    {job.state === "Paused" && (
                      <button
                        className="action-btn play"
                        onClick={() => handleAction("resume_job", job.id)}
                      >
                        ▶ Resume
                      </button>
                    )}
                    {job.state === "Failed" && (
                      <>
                        <button
                          className="action-btn play"
                          onClick={() => handleAction("retry_job", job.id)}
                        >
                          🔄 Retry
                        </button>
                        <button
                          className="action-btn warning"
                          onClick={() => {
                            setRefreshingId(job.id);
                            setNewUrl("");
                          }}
                        >
                          🔗 Refresh Link
                        </button>
                      </>
                    )}
                    {(job.state === "Active" || job.state === "Paused" || job.state === "Queued") && (
                      <button
                        className="action-btn danger"
                        onClick={() => handleAction("cancel_job", job.id)}
                      >
                        ✕ Cancel
                      </button>
                    )}
                  </div>
                </div>

                <div className="progress-container">
                  <progress
                    value={job.completed_bytes}
                    max={job.total_bytes || Math.max(job.completed_bytes, 100)}
                  />
                </div>

                <div className="job-stats">
                  <span>
                    {formatBytes(job.completed_bytes)} / {formatBytes(job.total_bytes)}
                    {percent ? ` (${percent}%)` : ""}
                  </span>
                  {job.state === "Active" && (
                    <span className="speed">⚡ {formatSpeed(job.speed_bytes_per_sec)}</span>
                  )}
                  {job.state === "Queued" && (
                    <span className="queued-indicator">⏳ Waiting in queue</span>
                  )}
                </div>

                {refreshingId === job.id && (
                  <div className="refresh-box">
                    <input
                      placeholder="Paste new refreshed download URL..."
                      value={newUrl}
                      onChange={(e) => setNewUrl(e.target.value)}
                    />
                    <button className="primary-btn" onClick={() => submitRefresh(job.id)}>
                      Update & Resume
                    </button>
                    <button className="action-btn" onClick={() => setRefreshingId(null)}>
                      Cancel
                    </button>
                  </div>
                )}
              </article>
            );
          })
        )}
      </section>

      {/* ZIP Archive Content Preview Modal */}
      {previewFiles && (
        <div className="modal-backdrop" onClick={() => setPreviewFiles(null)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2>📦 Archive Inspection: {previewArchiveName}</h2>
                <span className="modal-sub">
                  Direct range preview from remote server ({previewFiles.length} files detected)
                </span>
              </div>
              <button className="modal-close-btn" onClick={() => setPreviewFiles(null)}>
                ✕
              </button>
            </div>

            <div className="modal-body">
              <table className="archive-table">
                <thead>
                  <tr>
                    <th>Filename</th>
                    <th>Uncompressed</th>
                    <th>Compressed</th>
                  </tr>
                </thead>
                <tbody>
                  {previewFiles.map((file, idx) => (
                    <tr key={idx}>
                      <td className="archive-filename">
                        {file.is_dir ? "📁 " : "📄 "}
                        {file.name}
                      </td>
                      <td>{formatBytes(file.uncompressed_size)}</td>
                      <td>{formatBytes(file.compressed_size)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
