/**
 * Marks for the About tab's link tiles.
 *
 * Drawn rather than set as emoji: the emoji versions render at different
 * weights and colours depending on the fonts a player has, which made the
 * tiles look like a jumble. Stroke glyphs inherit `currentColor`; the two
 * brand marks are solid, the way those logos are meant to be drawn.
 */

const stroke = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
} as const;

export function GlobeIcon({ className = "" }: { className?: string }) {
  return (
    <svg {...stroke} className={className}>
      <circle cx="12" cy="12" r="9.5" />
      <path d="M2.5 12h19" />
      <path d="M12 2.5a14 14 0 0 1 0 19a14 14 0 0 1 0-19Z" />
    </svg>
  );
}

export function BookIcon({ className = "" }: { className?: string }) {
  return (
    <svg {...stroke} className={className}>
      <path d="M12 6.5C10.5 5 8.5 4.5 4 4.5v13c4.5 0 6.5.5 8 2 1.5-1.5 3.5-2 8-2v-13c-4.5 0-6.5.5-8 2Z" />
      <path d="M12 6.5v13" />
    </svg>
  );
}

export function BugIcon({ className = "" }: { className?: string }) {
  return (
    <svg {...stroke} className={className}>
      <rect x="8" y="7" width="8" height="13" rx="4" />
      <path d="M9.5 7a2.5 2.5 0 0 1 5 0" />
      <path d="M8 11H4.5M16 11h3.5M8 15.5H4.5M16 15.5h3.5M9 19l-2 2.5M15 19l2 2.5M9 7.5 7.5 5M15 7.5 16.5 5" />
    </svg>
  );
}

export function ChatIcon({ className = "" }: { className?: string }) {
  return (
    <svg {...stroke} className={className}>
      <path d="M20.5 12.5c0 4-3.8 7-8.5 7a10 10 0 0 1-2.6-.34l-4.9 1.6 1.6-3.7A6.6 6.6 0 0 1 3.5 12.5c0-3.9 3.8-7 8.5-7s8.5 3.1 8.5 7Z" />
      <path d="M9 12h.01M12 12h.01M15 12h.01" />
    </svg>
  );
}

export function GitHubIcon({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" className={className}>
      <path d="M12 .5C5.73.5.75 5.48.75 11.75c0 4.96 3.22 9.16 7.69 10.65.56.1.77-.24.77-.54v-2.1c-3.13.68-3.79-1.32-3.79-1.32-.51-1.3-1.25-1.65-1.25-1.65-1.02-.7.08-.68.08-.68 1.13.08 1.73 1.16 1.73 1.16 1 1.72 2.64 1.22 3.28.94.1-.73.39-1.23.71-1.51-2.5-.29-5.13-1.25-5.13-5.57 0-1.23.44-2.24 1.16-3.03-.12-.29-.5-1.43.11-2.99 0 0 .95-.3 3.1 1.16a10.7 10.7 0 0 1 5.64 0c2.15-1.46 3.09-1.16 3.09-1.16.62 1.56.23 2.7.12 2.99.72.79 1.16 1.8 1.16 3.03 0 4.33-2.64 5.28-5.15 5.56.4.35.77 1.04.77 2.1v3.11c0 .3.2.65.78.54a11.26 11.26 0 0 0 7.68-10.65C23.25 5.48 18.27.5 12 .5Z" />
    </svg>
  );
}

export function DiscordIcon({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" className={className}>
      <path d="M19.54 5.34A16.4 16.4 0 0 0 15.5 4.1l-.2.38c-.25.45-.53 1.04-.72 1.5a15.3 15.3 0 0 0-5.16 0c-.2-.46-.48-1.05-.73-1.5l-.2-.38a16.4 16.4 0 0 0-4.03 1.24C1.9 9.13 1.2 12.8 1.55 16.42a16.6 16.6 0 0 0 5.02 2.54c.4-.55.77-1.14 1.08-1.76a10.8 10.8 0 0 1-1.7-.82l.42-.33a11.6 11.6 0 0 0 9.94 0l.42.33c-.54.32-1.11.6-1.71.82.31.62.67 1.2 1.08 1.76a16.6 16.6 0 0 0 5.03-2.54c.42-4.2-.7-7.83-2.59-11.08ZM8.52 14.23c-.98 0-1.79-.9-1.79-2.01 0-1.11.79-2.02 1.79-2.02s1.81.91 1.79 2.02c0 1.1-.79 2.01-1.79 2.01Zm6.6 0c-.98 0-1.79-.9-1.79-2.01 0-1.11.79-2.02 1.79-2.02s1.8.91 1.79 2.02c0 1.1-.79 2.01-1.79 2.01Z" />
    </svg>
  );
}
