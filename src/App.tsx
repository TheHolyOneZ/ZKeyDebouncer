import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Trace, { type Pulse } from "./Trace";
import About from "./About";
import type { KeyEvent, Snapshot } from "./types";

type LogRow = { id: number; at: number; key: string; gapMs: number };

const MIN_MS = 5;
const MAX_MS = 150;
const DEFAULT_MS = 30;
const MAX_ROWS = 60;
const MAX_PULSES = 400;
const RATE_WINDOW_MS = 60_000;

const clock = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

function stamp(at: number) {
  return `${clock.format(at)}.${String(at % 1000).padStart(3, "0")}`;
}

function pct(value: number) {
  return `${((value - MIN_MS) / (MAX_MS - MIN_MS)) * 100}%`;
}

export default function App() {
  const [threshold, setThreshold] = useState(DEFAULT_MS);
  const [blocked, setBlocked] = useState(0);
  const [filtering, setFiltering] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [rows, setRows] = useState<LogRow[]>([]);
  const [recent, setRecent] = useState<number[]>([]);
  const [seen, setSeen] = useState(0);
  const [tab, setTab] = useState<"monitor" | "about">("monitor");

  const pulses = useRef<Pulse[]>([]);
  const rowId = useRef(0);

  useEffect(() => {
    invoke<Snapshot>("snapshot")
      .then((s) => {
        setThreshold(s.thresholdMs);
        setBlocked(s.blocked);
        setSeen(s.seen);
        setFiltering(s.filtering);
        setError(s.error);
      })
      .catch((e) => setError(String(e)));

    const subs = Promise.all([
      listen<KeyEvent>("key-event", ({ payload }) => {
        const at = Date.now();
        pulses.current.push({ t: at, blocked: payload.kind === "block" });
        if (pulses.current.length > MAX_PULSES) {
          pulses.current.splice(0, pulses.current.length - MAX_PULSES);
        }
        if (payload.kind !== "block") return;

        setBlocked(payload.total);
        setRecent((prev) => [...prev, at].filter((t) => at - t < RATE_WINDOW_MS));
        setRows((prev) =>
          [
            {
              id: rowId.current++,
              at,
              key: payload.key ?? "?",
              gapMs: payload.gapMs ?? 0,
            },
            ...prev,
          ].slice(0, MAX_ROWS)
        );
      }),
      listen<string>("hook-error", ({ payload }) => setError(payload)),
    ]).catch((e) => {
      setError(`the window could not subscribe to hook events (${e})`);
      return [] as Array<() => void>;
    });

    return () => {
      subs.then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  useEffect(() => {
    const id = setInterval(() => {
      setRecent((prev) => prev.filter((t) => Date.now() - t < RATE_WINDOW_MS));
      invoke<Snapshot>("snapshot")
        .then((s) => {
          setBlocked(s.blocked);
          setSeen(s.seen);
              if (s.error) setError(s.error);
        })
        .catch(() => {});
    }, 1000);
    return () => clearInterval(id);
  }, []);

  const commitThreshold = useCallback((ms: number) => {
    setThreshold(ms);
    invoke("set_threshold", { ms }).catch((e) => setError(String(e)));
  }, []);

  const toggle = useCallback(() => {
    const next = !filtering;
    setFiltering(next);
    invoke("set_filtering", { on: next }).catch((e) => setError(String(e)));
  }, [filtering]);

  const reset = useCallback(() => {
    invoke("reset_count")
      .then(() => {
        setBlocked(0);
        setRows([]);
        setRecent([]);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const appWindow = useMemo(() => getCurrentWindow(), []);

  return (
    <div className="app" data-paused={!filtering}>
      <header className="bar" data-tauri-drag-region>
        <h1 className="wordmark" data-tauri-drag-region>
          ZKeyDebouncer
        </h1>
        <nav className="tabs">
          <button
            className={tab === "monitor" ? "tab tab-on" : "tab"}
            onClick={() => setTab("monitor")}
          >
            Monitor
          </button>
          <button
            className={tab === "about" ? "tab tab-on" : "tab"}
            onClick={() => setTab("about")}
          >
            About
          </button>
        </nav>
        <div className="bar-buttons">
          <button
            className="win"
            onClick={() => appWindow.minimize()}
            title="Minimize"
            aria-label="Minimize"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <path d="M1 5h8" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          </button>
          <button
            className="win win-close"
            onClick={() => appWindow.close()}
            title="Close"
            aria-label="Close"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <path
                d="M1.6 1.6l6.8 6.8M8.4 1.6l-6.8 6.8"
                stroke="currentColor"
                strokeWidth="1.2"
              />
            </svg>
          </button>
        </div>
      </header>

      {error && (
        <p className="alert" role="alert">
          Keystrokes are no longer being filtered ({error}). Restart
          ZKeyDebouncer to reinstall the hook.
        </p>
      )}

      {tab === "about" ? (
        <About />
      ) : (
      <main className="body">
        <section className="sec" aria-live="polite">
          <p className="label">Bounces dropped</p>
          <div className="reading">
            <span className="count">{blocked.toLocaleString()}</span>
            <span className="rate">
              {recent.length === 0
                ? "0 in the last minute"
                : `${recent.length} in the last minute`}
            </span>
          </div>
          <div className="rule" aria-hidden="true" />
        </section>

        <section className="sec">
          <div className="well">
            <Trace pulses={pulses} paused={!filtering} />
          </div>
          <div className="caption">
            <span>Last 6 s</span>
            <span className="keys">
              <span className="k">Forwarded</span>
              <span className="k k-drop">Dropped</span>
            </span>
          </div>
        </section>

        <section className="sec">
          <div className="sec-head">
            <h2 className="label">Bounce window</h2>
            <span className="readout">
              {threshold}
              <em>ms</em>
            </span>
          </div>
          <input
            className="slider"
            style={{ "--pct": pct(threshold) } as React.CSSProperties}
            type="range"
            min={MIN_MS}
            max={MAX_MS}
            step={1}
            value={threshold}
            aria-label="Bounce window in milliseconds"
            onChange={(e) => commitThreshold(Number(e.target.value))}
          />
          <div className="scale">
            <span>{MIN_MS} ms</span>
            <span>{DEFAULT_MS} ms typical</span>
            <span>{MAX_MS} ms</span>
          </div>
          <p className="hint">
            A second press of the same key inside this window is dropped. Raise
            it if doubles still get through; lower it if deliberate fast repeats
            go missing.
          </p>
        </section>

        <section className="sec sec-log">
          <div className="sec-head">
            <h2 className="label">Dropped</h2>
            {rows.length > 0 && <span className="rate">last {rows.length}</span>}
          </div>
          {rows.length === 0 ? (
            <p className="empty">
              <b>Nothing dropped yet</b>
              {seen > 0
                ? `${seen.toLocaleString()} presses checked`
                : "Type normally. Bounces show up here."}
            </p>
          ) : (
            <ul className="log">
              {rows.map((r) => (
                <li key={r.id}>
                  <span className="cap">{r.key}</span>
                  <span className="log-time">{stamp(r.at)}</span>
                  <span className="log-gap">{r.gapMs} ms</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </main>
      )}

      <footer className="foot">
        <span className="state">{filtering ? "Filtering" : "Paused"}</span>
        <div className="foot-buttons">
          <button className="quiet" onClick={reset}>
            Reset
          </button>
          <button className="action" onClick={toggle}>
            {filtering ? "Pause filtering" : "Resume filtering"}
          </button>
        </div>
      </footer>
    </div>
  );
}
