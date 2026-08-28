import { useEffect, useRef } from "react";

export type Pulse = { t: number; blocked: boolean };

const SPAN_MS = 6000;

export default function Trace({
  pulses,
  paused,
}: {
  pulses: React.RefObject<Pulse[]>;
  paused: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let raf = 0;
    let lastPaint = 0;

    const token = (name: string) =>
      getComputedStyle(canvas).getPropertyValue(name).trim();

    const draw = (frame: number) => {
      raf = requestAnimationFrame(draw);

      if (motion.matches && frame - lastPaint < 250) return;
      lastPaint = frame;

      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w === 0 || h === 0) return;
      if (canvas.width !== Math.round(w * dpr)) {
        canvas.width = Math.round(w * dpr);
        canvas.height = Math.round(h * dpr);
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      const amber = token("--amber") || "#ffb020";
      const red = token("--red") || "#ff5f46";
      const fade = pausedRef.current ? 0.32 : 1;

      const t = Date.now();
      const base = h - 8;
      const top = 6;
      const xOf = (age: number) => Math.round(w * (1 - age / SPAN_MS)) + 0.5;

      ctx.globalAlpha = 0.22 * fade;
      ctx.strokeStyle = "#efe9df";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, base + 0.5);
      ctx.lineTo(w, base + 0.5);
      ctx.stroke();

      ctx.globalAlpha = 0.3 * fade;
      for (let s = 1; s <= SPAN_MS / 1000; s++) {
        const x = xOf((t % 1000) + s * 1000);
        if (x < 0) continue;
        ctx.beginPath();
        ctx.moveTo(x, base + 1);
        ctx.lineTo(x, base + 4);
        ctx.stroke();
      }

      for (const p of pulses.current ?? []) {
        const age = t - p.t;
        if (age < 0 || age > SPAN_MS) continue;
        const x = xOf(age);
        ctx.globalAlpha = fade;
        ctx.strokeStyle = p.blocked ? red : amber;
        ctx.lineWidth = p.blocked ? 2 : 1;

        if (p.blocked) {
          const height = (base - top) * 0.38;
          ctx.beginPath();
          ctx.moveTo(x, base);
          ctx.lineTo(x, base - height);
          ctx.stroke();

          ctx.beginPath();
          ctx.moveTo(x - 2.5, base - height - 4);
          ctx.lineTo(x + 2.5, base - height - 4);
          ctx.stroke();
        } else {
          ctx.beginPath();
          ctx.moveTo(x, base);
          ctx.lineTo(x, top);
          ctx.stroke();
        }
      }

      ctx.globalAlpha = 0.55 * fade;
      ctx.strokeStyle = amber;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(w - 0.5, top - 4);
      ctx.lineTo(w - 0.5, base);
      ctx.stroke();
      ctx.globalAlpha = 1;
    };

    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [pulses]);

  return <canvas ref={canvasRef} className="lane" aria-hidden="true" />;
}
