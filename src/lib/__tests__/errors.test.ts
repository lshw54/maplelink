import { describe, expect, it } from "vitest";
import { errorMessage } from "../errors";
import type { ErrorDto } from "../types";

describe("errorMessage", () => {
  it("never renders an object as [object Object]", () => {
    // The reported bug: a red box reading exactly this, in every locale,
    // with nothing to act on.
    const dto: ErrorDto = {
      code: "CLIENT_MANIFEST_FAILED",
      message: "product list request failed: error sending request",
      category: "network",
    };
    const shown = errorMessage(dto);
    expect(shown).not.toContain("[object Object]");
    expect(shown).toContain("product list request failed");
    // The code is what makes a screenshot reportable.
    expect(shown).toContain("CLIENT_MANIFEST_FAILED");
  });

  it("falls back to the code when the backend sent no message", () => {
    expect(errorMessage({ code: "SYS_PATH_ERROR", message: "", category: "process" })).toBe(
      "SYS_PATH_ERROR",
    );
    expect(
      errorMessage({ code: "SYS_PATH_ERROR", message: "", category: "process", details: "C:\\x" }),
    ).toBe("SYS_PATH_ERROR: C:\\x");
  });

  it("reads an Error and a plain string", () => {
    expect(errorMessage(new Error("disk full"))).toBe("disk full");
    expect(errorMessage("game path is not set")).toBe("game path is not set");
  });

  it("serialises an object it does not recognise rather than hiding it", () => {
    expect(errorMessage({ reason: "odd", attempt: 2 })).toBe('{"reason":"odd","attempt":2}');
  });

  it("survives the awkward values a rejection can carry", () => {
    expect(errorMessage(undefined)).toBe("undefined");
    expect(errorMessage(null)).toBe("null");
    expect(errorMessage(404)).toBe("404");
    expect(errorMessage("   ")).toBe("   ");
    expect(errorMessage(new Error(""))).toBe("Error");
    expect(errorMessage({})).toBe("{}");

    // A circular object cannot be serialised; it must still not throw.
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(() => errorMessage(loop)).not.toThrow();
  });
});
