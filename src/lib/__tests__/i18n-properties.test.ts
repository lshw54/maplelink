/**
 * Property-based tests for i18n locale resource files.
 *
 * Feature: maplelink-rewrite, Property 16: Locale resource files key completeness
 * Feature: maplelink-rewrite, Property 17: Error codes have translations in all locales
 */
import { describe, it, expect } from "vitest";
import * as fc from "fast-check";
import enUS from "../../locales/en-US.json";
import zhTW from "../../locales/zh-TW.json";
import zhCN from "../../locales/zh-CN.json";

type LocaleMap = Record<string, string>;

const locales: Record<string, LocaleMap> = {
  "en-US": enUS,
  "zh-TW": zhTW,
  "zh-CN": zhCN,
};

const localeNames = Object.keys(locales);

/**
 * All known backend error codes derived from the Rust error type hierarchy.
 * These correspond to the `ErrorDto.code` values produced by `From<AppError> for ErrorDto`.
 */
const ERROR_CODES: string[] = [
  // AuthError variants
  "AUTH_INVALID_CREDENTIALS",
  "AUTH_SESSION_EXPIRED",
  "AUTH_TOTP_FAILED",
  "AUTH_QR_EXPIRED",
  "AUTH_NOT_AUTHENTICATED",
  // NetworkError variants
  "NET_CONNECTION_FAILED",
  "NET_TIMEOUT",
  "NET_HTTP_ERROR",
  // FsError variants
  "FS_NOT_FOUND",
  "FS_PERMISSION_DENIED",
  "FS_IO",
  // ProcessError variants
  "PROC_SPAWN_FAILED",
  // ConfigError variants
  "CFG_PARSE_ERROR",
  "CFG_WRITE_ERROR",
  // UpdateError variants
  "UPD_CHECK_FAILED",
  "UPD_DOWNLOAD_FAILED",
  "UPD_CORRUPT_DOWNLOAD",
];

// ---------------------------------------------------------------------------
// Property 16: Locale resource files key completeness
// ---------------------------------------------------------------------------
describe("Property 16: Locale resource files key completeness", () => {
  it("all locale files have identical key sets", () => {
    const keySets: Record<string, Set<string>> = {};
    for (const name of localeNames) {
      keySets[name] = new Set(Object.keys(locales[name] as LocaleMap));
    }

    // Use fast-check to pick arbitrary pairs of locales and verify key equality
    fc.assert(
      fc.property(
        fc.constantFrom(...localeNames),
        fc.constantFrom(...localeNames),
        (localeA, localeB) => {
          const keysA = keySets[localeA] as Set<string>;
          const keysB = keySets[localeB] as Set<string>;

          const missingInB = [...keysA].filter((k) => !keysB.has(k));
          const missingInA = [...keysB].filter((k) => !keysA.has(k));

          expect(missingInB, `${localeB} is missing keys present in ${localeA}`).toEqual([]);
          expect(missingInA, `${localeA} is missing keys present in ${localeB}`).toEqual([]);
        },
      ),
      { numRuns: 100 },
    );
  });

  it("no locale file has empty string values", () => {
    fc.assert(
      fc.property(fc.constantFrom(...localeNames), (localeName) => {
        const locale = locales[localeName] as LocaleMap;
        const entries = Object.entries(locale);
        for (const [key, value] of entries) {
          expect(value.trim(), `${localeName}["${key}"] is empty`).not.toBe("");
        }
      }),
      { numRuns: 100 },
    );
  });
});

// ---------------------------------------------------------------------------
// Property 17: Error codes have translations in all locales
// ---------------------------------------------------------------------------
describe("Property 17: Error codes have translations in all locales", () => {
  it("every backend error code has a translation in every locale", () => {
    fc.assert(
      fc.property(
        fc.constantFrom(...ERROR_CODES),
        fc.constantFrom(...localeNames),
        (errorCode, localeName) => {
          const translationKey = `errors.${errorCode}`;
          const locale = locales[localeName] as LocaleMap;

          const value = locale[translationKey];

          expect(
            value,
            `${localeName} is missing translation for "${translationKey}"`,
          ).toBeDefined();

          expect((value ?? "").trim(), `${localeName}["${translationKey}"] is empty`).not.toBe("");
        },
      ),
      { numRuns: 100 },
    );
  });
});
