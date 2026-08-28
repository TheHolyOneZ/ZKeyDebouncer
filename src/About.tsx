import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";

type Row = { label: string; sub?: string; url: string };

const AUTHORS: Row[] = [
  {
    label: "TheHolyOneZ",
    sub: "Author",
    url: "https://github.com/TheHolyOneZ",
  },
  {
    label: "Quix-ux681",
    sub: "Co-author",
    url: "https://github.com/Quix-ux681",
  },
];

const LINKS: Row[] = [
  { label: "Project homepage", url: "https://zsync.eu/zkeydebouncer/" },
  { label: "Source code", url: "https://github.com/TheHolyOneZ/ZKeyDebouncer" },
  { label: "More projects", url: "https://zsync.eu/" },
];

function display(url: string) {
  return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

function LinkRow({ row }: { row: Row }) {
  return (
    <button
      className="link"
      onClick={() => openUrl(row.url)}
      title={`Open ${row.url}`}
    >
      <span className="link-label">
        {row.label}
        {row.sub && <em>{row.sub}</em>}
      </span>
      <span className="link-url">
        {display(row.url)}
        <svg width="7" height="7" viewBox="0 0 8 8" aria-hidden="true">
          <path
            d="M1.5 6.5L6.5 1.5M2.6 1.5h3.9v3.9"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.1"
          />
        </svg>
      </span>
    </button>
  );
}

export default function About() {
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  return (
    <div className="about">
      <section className="sec">
        <p className="label">About</p>
        <div className="reading">
          <span className="about-name">ZKeyDebouncer</span>
        </div>
        <p className="about-blurb">
          Filters mechanical-keyboard chatter system-wide, so one physical press
          registers once.
        </p>
        <p className="about-meta">
          {version ? `Version ${version}` : "Version unknown"} · GPL-3.0-or-later
        </p>
      </section>

      <section className="sec">
        <h2 className="label">Authors</h2>
        <div className="links">
          {AUTHORS.map((row) => (
            <LinkRow key={row.url} row={row} />
          ))}
        </div>
      </section>

      <section className="sec">
        <h2 className="label">Links</h2>
        <div className="links">
          {LINKS.map((row) => (
            <LinkRow key={row.url} row={row} />
          ))}
        </div>
      </section>

    </div>
  );
}
