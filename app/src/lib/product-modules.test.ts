import { describe, expect, it } from "vitest";

import {
  getProductModuleFromPath,
  qaPaths,
  resolveModuleApiBaseUrl,
  rewriteLegacyQaPath,
  stripProductApiSuffix,
} from "@/lib/product-modules";

describe("product modules helpers", () => {
  it("resolves the active product module from the pathname", () => {
    expect(getProductModuleFromPath("/qa/projects/demo")).toBe("qa");
    expect(getProductModuleFromPath("/atlas")).toBe("atlas");
    expect(getProductModuleFromPath("/o11y")).toBe("o11y");
    expect(getProductModuleFromPath("/projects/demo")).toBe("qa");
  });

  it("builds QA paths with the modular prefix", () => {
    expect(qaPaths.home()).toBe("/qa");
    expect(qaPaths.project("project-1")).toBe("/qa/projects/project-1");
    expect(qaPaths.specDiff("project-1", "spec-1")).toBe("/qa/projects/project-1/specs/spec-1/diff");
  });

  it("rewrites legacy QA paths to the modular namespace", () => {
    expect(rewriteLegacyQaPath("/")).toBe("/qa");
    expect(rewriteLegacyQaPath("/projects")).toBe("/qa");
    expect(rewriteLegacyQaPath("/projects/project-1")).toBe("/qa/projects/project-1");
    expect(rewriteLegacyQaPath("/project/project-1/specs/spec-1/editor")).toBe("/qa/projects/project-1/specs/spec-1/editor");
    expect(rewriteLegacyQaPath("/atlas")).toBeNull();
  });

  it("keeps QA API compatibility with the current backend base", () => {
    expect(resolveModuleApiBaseUrl("http://localhost:5588", "qa")).toBe("http://localhost:5588/api/v1");
    expect(resolveModuleApiBaseUrl("http://localhost:5588/api/v1", "qa")).toBe("http://localhost:5588/api/v1");
  });

  it("can resolve future modular API bases when needed", () => {
    expect(resolveModuleApiBaseUrl("http://localhost:5588", "atlas")).toBe("http://localhost:5588/atlas/api/v1");
    expect(resolveModuleApiBaseUrl("http://localhost:5588", "o11y")).toBe("http://localhost:5588/o11y/api/v1");
    expect(resolveModuleApiBaseUrl("http://localhost:5588", "qa", { preferLegacyQa: false })).toBe("http://localhost:5588/qa/api/v1");
  });

  it("strips legacy and modular API suffixes", () => {
    expect(stripProductApiSuffix("http://localhost:5588/api/v1")).toBe("http://localhost:5588");
    expect(stripProductApiSuffix("http://localhost:5588/qa/api/v1")).toBe("http://localhost:5588");
    expect(stripProductApiSuffix("http://localhost:5588/o11y/api/v1")).toBe("http://localhost:5588");
  });
});
