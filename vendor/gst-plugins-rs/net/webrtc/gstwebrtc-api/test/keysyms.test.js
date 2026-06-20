import { expect } from "chai";

import getKeysymString from "../src/keysyms.js";

describe("getKeysymString tests", () => {
  it("should return mapped keySym value from codesToKeySyms", () => {
    const key = "Shift";
    const code = "ShiftLeft";
    const result = getKeysymString(key, code);
    expect(result).to.equal("Shift_L");
  });

  it("should return the keysym for a single-character key from uniToKeySyms", () => {
    const key = "A";
    const code = "KeyA";
    const result = getKeysymString(key, code);
    expect(result).to.equal("A");
  });

  it("should handle non-ASCII characters from uniToKeySyms", () => {
    const key = "ф";
    const code = "KeyA";
    const result = getKeysymString(key, code);
    expect(result).to.equal("Cyrillic_ef");
  });

  it("should return the valid keySym from knownKeysyms", () => {
    const key = "Tab";
    const code = "Tab";
    const result = getKeysymString(key, code);
    expect(result).to.equal(code);
  });

  it("should return the default keySym if no match is found", () => {
    const key = "InvalidKey";
    const code = "InvalidCode";
    const result = getKeysymString(key, code);
    expect(result).to.equal("Unidentified");
  });
});
