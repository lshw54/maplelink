import { useState, useEffect, useRef } from "react";

export function StatusBar() {
  const [online, setOnline] = useState(false);
  const [ms, setMs] = useState<number | null>(null);
  const [beat, setBeat] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    async function heartbeat() {
      const t0 = performance.now();
      try {
        await fetch("https://tw.beanfun.com/favicon.ico", {
          mode: "no-cors",
          cache: "no-store",
        });
        const elapsed = Math.round(performance.now() - t0);
        setOnline(true);
        setMs(elapsed);
        setBeat(true);
        setTimeout(() => setBeat(false), 400);
      } catch {
        setOnline(false);
        setMs(null);
      }
    }

    heartbeat();
    intervalRef.current = setInterval(heartbeat, 5000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  return (
    <div className="flex shrink-0 items-center justify-center gap-1.5 px-2 py-0.5 font-mono text-[12px] text-text-dim">
      <span
        className={`h-1.5 w-1.5 shrink-0 rounded-full transition-colors ${
          online ? "bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.4)]" : "bg-[var(--danger)]"
        } ${beat ? "animate-[hbeat_0.4s_ease]" : ""}`}
      />
      <span>{online ? "ONLINE" : "OFFLINE"}</span>
      <span className="text-[12px] text-text-faint">{ms !== null ? `${ms}ms` : "--"}</span>
    </div>
  );
}
