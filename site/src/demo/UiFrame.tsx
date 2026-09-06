import React, { useEffect, useRef, useState, type ReactNode } from "react";
import styles from "./UiFrame.module.css";

/**
 * Scales a piece of UI drawn at its natural pixel size down to whatever width
 * the page gives it, never up. The frame is sized purely by aspect ratio; the
 * content is absolutely positioned on top and transform-scaled, so it can never
 * stretch the layout around it.
 */
export default function UiFrame({ width, height, children }: { width: number; height: number; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const fit = () => setScale(Math.min(1, el.clientWidth / width));
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [width]);
  return (
    <div ref={frame} className={styles.frame} style={{ aspectRatio: `${width} / ${height}`, maxWidth: width }}>
      <div className={styles.content} style={{ width, height, transform: `scale(${scale})` }}>
        {children}
      </div>
    </div>
  );
}
