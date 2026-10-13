import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

type Job = { id: string; name: string; status: string; progress: number };

function App() {
  const [url, setUrl] = useState("");
  const [jobs, setJobs] = useState<Job[]>([]);
  const add = () => {
    try {
      const parsed = new URL(url);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("HTTP(S) only");
      setJobs((current) => [...current, { id: crypto.randomUUID(), name: parsed.pathname.split("/").pop() || parsed.hostname, status: "Queued", progress: 0 }]);
      setUrl("");
    } catch {
      alert("Enter a valid HTTP or HTTPS URL.");
    }
  };
  return <main>
    <header><h1>Hopper</h1><span>Download Manager</span></header>
    <section className="add"><input value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} placeholder="Paste a download URL" /><button onClick={add}>Add download</button></section>
    <nav><button className="active">All downloads</button><button>In progress</button><button>Completed</button></nav>
    <section className="list">{jobs.length === 0 ? <p className="empty">No downloads yet. Add a URL to get started.</p> : jobs.map((job) => <article key={job.id}><div><strong>{job.name}</strong><small>{job.status}</small></div><progress value={job.progress} max="100" /></article>)}</section>
  </main>;
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
