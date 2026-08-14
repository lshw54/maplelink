/**
 * Blur applied to account names when "hide account names" is on, so a stream or
 * a screenshot doesn't leak them. Hovering reveals the name for the person
 * actually sitting there.
 *
 * Kept in one place: it was written out twice before, and the account header on
 * the main page was simply missed.
 */
export const MASK_CLASS = "blur-[5px] transition-[filter] duration-150 select-none hover:blur-none";
