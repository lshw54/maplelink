import React, { type ReactNode } from "react";
import clsx from "clsx";
import styles from "./UiCoach.module.css";

/** The one-line prompt under a demo: what to click next, or that it is done. */
export default function UiCoach({ done, action, children }: { done?: boolean; action?: ReactNode; children: ReactNode }) {
  return (
    <div className={clsx(styles.coach, done && styles.done)}>
      <span className={styles.text}>{children}</span>
      {action}
    </div>
  );
}
