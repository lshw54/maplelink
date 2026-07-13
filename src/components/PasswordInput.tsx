import { useState } from "react";

/** A password `<input>` with a show/hide eye toggle on the right.
 *  Pass the same props as a normal input; give the className enough right
 *  padding (e.g. `pr-9`) so text doesn't run under the eye button. */
export function PasswordInput({
  className = "",
  ...props
}: React.InputHTMLAttributes<HTMLInputElement>) {
  const [show, setShow] = useState(false);
  return (
    <div className="relative">
      <input {...props} type={show ? "text" : "password"} className={className} />
      <button
        type="button"
        tabIndex={-1}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setShow((s) => !s)}
        aria-label={show ? "Hide password" : "Show password"}
        className="absolute top-1/2 right-2.5 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-text-faint transition-colors hover:text-accent"
      >
        {show ? (
          <svg
            width="15"
            height="15"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M2 2l12 12" />
            <path d="M6.5 6.6a2 2 0 0 0 2.8 2.8" />
            <path d="M4.2 4.5C2.9 5.4 1.9 6.6 1.3 8c1.2 2.8 3.8 4.7 6.7 4.7 1.2 0 2.3-.3 3.3-.8" />
            <path d="M9.9 3.5C9.3 3.3 8.7 3.3 8 3.3c-.2 0 0 0 0 0m4.5 1.9c.8.7 1.5 1.6 2 2.8-.4.9-.9 1.7-1.6 2.3" />
          </svg>
        ) : (
          <svg
            width="15"
            height="15"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M1.3 8C2.5 5.2 5.1 3.3 8 3.3s5.5 1.9 6.7 4.7c-1.2 2.8-3.8 4.7-6.7 4.7S2.5 10.8 1.3 8Z" />
            <circle cx="8" cy="8" r="2" />
          </svg>
        )}
      </button>
    </div>
  );
}
