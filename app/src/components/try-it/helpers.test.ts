import { describe, expect, it } from "vitest";

import { buildUrl } from "./helpers";

describe("buildUrl", () => {
  it("preserves the server path prefix for OpenAPI routes", () => {
    expect(
      buildUrl("https://hml.runvibe.dev/qrud", "/users", [], [])
    ).toBe("https://hml.runvibe.dev/qrud/users");
  });

  it("handles servers without a path prefix", () => {
    expect(
      buildUrl("https://hml.runvibe.dev", "/users", [], [])
    ).toBe("https://hml.runvibe.dev/users");
  });

  it("applies path params and query params on top of the server prefix", () => {
    expect(
      buildUrl(
        "https://hml.runvibe.dev/qrud/",
        "/users/{userId}",
        [{ key: "userId", value: "abc 123" }],
        [{ key: "expand", value: "teams" }]
      )
    ).toBe("https://hml.runvibe.dev/qrud/users/abc%20123?expand=teams");
  });
});
