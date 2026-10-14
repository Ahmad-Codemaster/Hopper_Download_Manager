export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || Number.isNaN(bytes)) return "Unknown";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function formatSpeed(bytesPerSecond: number | undefined): string {
  return bytesPerSecond && bytesPerSecond > 0 ? `${formatBytes(bytesPerSecond)}/s` : "Unknown";
}

export function formatEta(completed: number, total: number | null, speed: number | undefined): string | null {
  if (!total || !speed || speed <= 0 || completed >= total) return null;
  const seconds = Math.ceil((total - completed) / speed);
  if (seconds < 60) return `${seconds}s left`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}m ${seconds % 60}s left`;
}

export function sourceHost(source: string): string {
  try {
    return new URL(source).host;
  } catch {
    return "Unknown source";
  }
}

export function fileKind(destination: string): "video" | "audio" | "archive" | "image" | "document" | "file" {
  const extension = destination.split(".").pop()?.toLowerCase();
  if (["mp4", "mkv", "webm", "mov", "avi"].includes(extension ?? "")) return "video";
  if (["mp3", "wav", "m4a", "flac", "ogg"].includes(extension ?? "")) return "audio";
  if (["zip", "tar", "gz", "7z", "rar"].includes(extension ?? "")) return "archive";
  if (["png", "jpg", "jpeg", "gif", "webp"].includes(extension ?? "")) return "image";
  if (["pdf", "doc", "docx", "txt"].includes(extension ?? "")) return "document";
  return "file";
}
