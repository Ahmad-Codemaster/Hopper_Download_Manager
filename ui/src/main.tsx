import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";
import { createAndStart, listJobs, type Job } from "./lib/api";

function App() {
  const [url, setUrl] = useState("");
  const [jobs, setJobs] = useState<Job[]>([]);
  useEffect(() => {
    listJobs().then(setJobs).catch((error) => console.error("Could not load downloads", error));
  }, []);
  const add = async () => {
    try {
      const parsed = new URL(url);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("HTTP(S) only");
      const id = await createAndStart(parsed.toString(), parsed.pathname.split("/").pop() || "download");
      setJobs((current) => [...current, { id: { 0: id }, destination: parsed.pathname.split("/").pop() || "download", state: "Active", completed_bytes: 0 }]);
      setUrl("");
    } catch (error) {
      console.error(error);
      alert("Enter a valid HTTP or HTTPS URL.");
    }
  };
  return <main>
    <header><h1>Hopper</h1><span>Download Manager</span></header>
    <section className="add"><input value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} placeholder="Paste a download URL" /><button onClick={add}>Add download</button></section>
    <nav><button className="active">All downloads</button><button>In progress</button><button>Completed</button></nav>
    <section className="list">{jobs.length === 0 ? <p className="empty">No downloads yet. Add a URL to get started.</p> : jobs.map((job) => <article key={job.id[0]}><div><strong>{job.destination}</strong><small>{job.state}</small></div><progress value={job.completed_bytes} max="100" /></article>)}</section>
  </main>;
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
