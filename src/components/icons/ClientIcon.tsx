/**
 * The client manager's mark: an arrow coming down into a tray.
 *
 * Drawn rather than set as an emoji — the title bar sits next to glyphs the
 * app already ships, and an emoji there renders differently (or not at all)
 * depending on the fonts a player has.
 */
export function ClientIcon({ className = "" }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      {/* the arrow */}
      <path d="M8 2v7" />
      <path d="M5 6.5 8 9.5l3-3" />
      {/* the tray it lands in */}
      <path d="M2.5 10.5v1.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5v-1.5" />
    </svg>
  );
}
