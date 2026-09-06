import React, { type ReactNode } from "react";
import NoticeBar from "../demo/NoticeBar";

/** Wraps every page: the announcement ticker sits above the navbar site-wide. */
export default function Root({ children }: { children: ReactNode }) {
  return (
    <>
      <NoticeBar />
      {children}
    </>
  );
}
