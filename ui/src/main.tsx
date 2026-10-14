import { StrictMode, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { Download, Plus, Search, Settings } from "lucide-react";
import "./styles.css";
import { createAndStart, getMaxConcurrent, getSpeedLimit, listJobs, onProgress, previewZipArchive, setMaxConcurrent, setSpeedLimit, controlJob, type Job, type ZipEntryPreview } from "./lib/api";
import { applyJobEvent, filterJobs, getJobCounts, type Category } from "./lib/jobs";
import { formatBytes } from "./lib/format";
import { DownloadDetails } from "./components/DownloadDetails";
import { DownloadRow } from "./components/DownloadRow";

const categories: Array<[Category, string]> = [["all", "All downloads"], ["downloading", "Downloading"], ["queued", "Queued"], ["completed", "Completed"], ["failed", "Failed"], ["paused", "Paused"]];

function App() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [category, setCategory] = useState<Category>("all");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [newUrl, setNewUrl] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [speedLimit, setSpeedLimitState] = useState(0);
  const [maxConcurrent, setMaxConcurrentState] = useState(3);
  const [preview, setPreview] = useState<{ name: string; files: ZipEntryPreview[] } | null>(null);
  const [pendingActions, setPendingActions] = useState<Set<string>>(new Set());

  const reload = () => listJobs().then(setJobs).catch((e) => setError(String(e)));
  useEffect(() => {
    reload();
    getSpeedLimit().then(setSpeedLimitState).catch(() => undefined);
    getMaxConcurrent().then(setMaxConcurrentState).catch(() => undefined);
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onProgress((event) => setJobs((current) => applyJobEvent(current, event))).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const counts = getJobCounts(jobs);
  const visibleJobs = useMemo(() => filterJobs(jobs, query, category), [jobs, query, category]);
  const selected = jobs.find((job) => job.id === selectedId);

  const action = async (command: "start_job" | "pause_job" | "resume_job" | "cancel_job" | "retry_job", id: string) => {
    if (pendingActions.has(id)) return;
    setPendingActions((current) => new Set(current).add(id));
    try { setError(null); await controlJob(command, id); await reload(); } catch (e) { setError(`Could not ${command.replace("_job", "").replace("_", " ")} download: ${String(e)}`); } finally { setPendingActions((current) => { const next = new Set(current); next.delete(id); return next; }); }
  };
  const add = async () => {
    try {
      const parsed = new URL(newUrl);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("HTTP(S) URLs only");
      if (parsed.username || parsed.password) throw new Error("URLs with embedded credentials are not supported");
      const destination = parsed.pathname.split("/").filter(Boolean).pop() || "download";
      await createAndStart(parsed.toString(), destination);
      setNewUrl(""); setShowAdd(false); await reload();
    } catch (e) { setError(`Could not add download: ${String(e)}`); }
  };
  const previewArchive = async (job: Job) => {
    if (!job.total_bytes) return;
    try { setPreview({ name: job.destination, files: await previewZipArchive(job.source, job.total_bytes) }); } catch (e) { setError(`Could not preview archive: ${String(e)}`); }
  };

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Download size={18} /></div><div><strong>Hopper</strong><span>Download manager</span></div></div>
      <button className="add-button" onClick={() => setShowAdd(true)}><Plus size={17} /> Add URL</button>
      <nav className="side-nav" aria-label="Download categories">{categories.map(([key, label]) => <button key={key} className={category === key ? "selected" : ""} onClick={() => { setCategory(key); setSelectedId(null); }}><span>{label}</span><b>{counts[key]}</b></button>)}</nav>
      <div className="sidebar-footer"><Settings size={16} /> <span>Transfer settings</span></div>
    </aside>
    <main className="workspace">
      <header className="title-bar"><div><span className="eyebrow">Library</span><h1>{selected ? "Download details" : categories.find(([key]) => key === category)?.[1]}</h1></div>{!selected && <div className="header-settings"><label>Speed <select value={speedLimit} onChange={(e) => { const value = Number(e.target.value); setSpeedLimitState(value); setSpeedLimit(value).catch((err) => setError(String(err))); }}><option value={0}>Unlimited</option><option value={1048576}>1 MB/s</option><option value={5242880}>5 MB/s</option><option value={10485760}>10 MB/s</option></select></label><label>Active <select value={maxConcurrent} onChange={(e) => { const value = Number(e.target.value); setMaxConcurrentState(value); setMaxConcurrent(value).catch((err) => setError(String(err))); }}><option value={1}>1</option><option value={3}>3</option><option value={5}>5</option></select></label></div>}</header>
      {error && <div className="error-notice" role="alert">{error}<button onClick={() => setError(null)}>Dismiss</button></div>}
      {selected ? <DownloadDetails job={selected} pendingAction={pendingActions.has(selected.id)} onBack={() => setSelectedId(null)} onAction={(command) => action(command, selected.id)} /> : <><div className="toolbar"><div className="search"><Search size={16} /><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search downloads" aria-label="Search downloads" /></div><span className="result-count">{visibleJobs.length} items</span></div><section className="download-list">{visibleJobs.length === 0 ? <div className="empty-state"><Download size={30} /><h2>{query ? "No matching downloads" : "No downloads yet"}</h2><p>{query ? "Try a different search." : "Add a URL to start your first transfer."}</p><button className="secondary-button" onClick={() => setShowAdd(true)}>Add URL</button></div> : visibleJobs.map((job) => <DownloadRow key={job.id} job={job} pendingAction={pendingActions.has(job.id)} onSelect={() => setSelectedId(job.id)} onAction={(command) => action(command, job.id)} onPreview={job.destination.toLowerCase().endsWith(".zip") ? () => previewArchive(job) : undefined} />)}</section></>}
    </main>
    {showAdd && <div className="modal-backdrop" onClick={() => setShowAdd(false)}><div className="dialog" onClick={(e) => e.stopPropagation()}><h2>Add download</h2><p>Paste a direct HTTP or HTTPS URL.</p><input autoFocus value={newUrl} onChange={(e) => setNewUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} placeholder="https://example.com/file.zip" /><div className="dialog-actions"><button className="secondary-button" onClick={() => setShowAdd(false)}>Cancel</button><button className="primary-button" onClick={add}>Start download</button></div></div></div>}
    {preview && <div className="modal-backdrop" onClick={() => setPreview(null)}><div className="dialog archive-dialog" onClick={(e) => e.stopPropagation()}><h2>{preview.name}</h2><p>{preview.files.length} entries detected</p><div className="archive-list">{preview.files.map((file) => <div key={file.name}><span>{file.is_dir ? "Folder" : file.name}</span><span>{formatBytes(file.uncompressed_size)}</span></div>)}</div><button className="secondary-button" onClick={() => setPreview(null)}>Close</button></div></div>}
  </div>;
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
