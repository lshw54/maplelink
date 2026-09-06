/**
 * Attributes that keep the browser and password managers out of the demo
 * fields, so they behave like the app's own inputs: no autofill dropdown, no
 * spell-check underline, no vault icon. Bind with `v-bind="NATIVE_INPUT"`.
 *
 * Secret fields are plain text inputs masked with CSS (`.ml-secret`) rather
 * than `type="password"`: a real password field makes Chrome and Edge offer to
 * save whatever was typed into a demo.
 */
export const NATIVE_INPUT = {
  autocomplete: "off",
  autocorrect: "off",
  autocapitalize: "off",
  spellcheck: "false",
  "data-form-type": "other",
  "data-lpignore": "true",
  "data-1p-ignore": "",
  "data-bwignore": "true",
} as const;
