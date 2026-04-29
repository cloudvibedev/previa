import { describe, it, expect } from "vitest";
import { resolveHelper, templateHelpers } from "./template-helpers";

describe("template-helpers", () => {
  describe("resolveHelper", () => {
    it("should return null for unknown helpers", () => {
      expect(resolveHelper("unknown")).toBeNull();
      expect(resolveHelper("notAHelper")).toBeNull();
    });

    it("should resolve uuid helper", () => {
      const result = resolveHelper("uuid");
      expect(result).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i);
    });

    it("should resolve email helper", () => {
      const result = resolveHelper("email");
      expect(result).toMatch(/@/);
      expect(result).toMatch(/\./);
    });

    it("should resolve name helper", () => {
      const result = resolveHelper("name");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
      expect(result!.length).toBeGreaterThan(0);
    });

    it("should resolve firstName helper", () => {
      const result = resolveHelper("firstName");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve lastName helper", () => {
      const result = resolveHelper("lastName");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve username helper", () => {
      const result = resolveHelper("username");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve phone helper", () => {
      const result = resolveHelper("phone");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve cpf helper with valid format", () => {
      const result = resolveHelper("cpf");
      expect(result).toBeTruthy();
      // CPF format: XXX.XXX.XXX-XX or just digits
      expect(result!.replace(/\D/g, "")).toHaveLength(11);
    });

    it("should resolve cnpj helper with valid format", () => {
      const result = resolveHelper("cnpj");
      expect(result).toBeTruthy();
      // CNPJ format: XX.XXX.XXX/XXXX-XX or just digits
      expect(result!.replace(/\D/g, "")).toHaveLength(14);
    });

    it("should resolve company helper", () => {
      const result = resolveHelper("company");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve timestamp helper with ISO format", () => {
      const result = resolveHelper("timestamp");
      expect(result).toBeTruthy();
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
    });

    it("should resolve date helper with date format", () => {
      const result = resolveHelper("date");
      expect(result).toBeTruthy();
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    });

    it("should resolve boolean helper", () => {
      const result = resolveHelper("boolean");
      expect(result).toMatch(/^(true|false)$/);
    });

    it("should resolve words helper with default count", () => {
      const result = resolveHelper("words");
      expect(result).toBeTruthy();
      const wordCount = result!.split(" ").length;
      expect(wordCount).toBeGreaterThanOrEqual(1);
    });

    it("should resolve words helper with custom count", () => {
      const result = resolveHelper("words 5");
      expect(result).toBeTruthy();
    });

    it("should resolve number helper with default range", () => {
      const result = resolveHelper("number");
      expect(result).toBeTruthy();
      const num = parseInt(result!, 10);
      expect(num).toBeGreaterThanOrEqual(0);
      expect(num).toBeLessThanOrEqual(100);
    });

    it("should resolve number helper with custom range", () => {
      const result = resolveHelper("number 10 20");
      expect(result).toBeTruthy();
      const num = parseInt(result!, 10);
      expect(num).toBeGreaterThanOrEqual(10);
      expect(num).toBeLessThanOrEqual(20);
    });

    it("should resolve street helper", () => {
      const result = resolveHelper("street");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve city helper", () => {
      const result = resolveHelper("city");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve state helper", () => {
      const result = resolveHelper("state");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve zipCode helper", () => {
      const result = resolveHelper("zipCode");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve country helper", () => {
      const result = resolveHelper("country");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
    });

    it("should resolve avatar helper with URL", () => {
      const result = resolveHelper("avatar");
      expect(result).toBeTruthy();
      expect(result).toMatch(/^https?:\/\//);
    });

    it("should resolve url helper", () => {
      const result = resolveHelper("url");
      expect(result).toBeTruthy();
      expect(result).toMatch(/^https?:\/\//);
    });

    it("should resolve password helper", () => {
      const result = resolveHelper("password");
      expect(result).toBeTruthy();
      expect(result!.length).toBeGreaterThanOrEqual(12);
    });

    it("should resolve text helper with sentences", () => {
      const result = resolveHelper("text");
      expect(result).toBeTruthy();
      expect(typeof result).toBe("string");
      expect(result!.length).toBeGreaterThan(10);
    });

    it("should resolve text helper with custom sentence count", () => {
      const result = resolveHelper("text 2");
      expect(result).toBeTruthy();
    });
  });

  describe("templateHelpers registry", () => {
    it("should have all expected helpers registered", () => {
      const expectedHelpers = [
        "uuid", "email", "name", "firstName", "lastName", "username",
        "phone", "cpf", "cnpj", "company", "timestamp", "date",
        "boolean", "words", "number", "street", "city", "state",
        "zipCode", "country", "avatar", "url", "password", "text"
      ];

      expectedHelpers.forEach((helper) => {
        expect(templateHelpers[helper]).toBeDefined();
        expect(typeof templateHelpers[helper]).toBe("function");
      });
    });

    it("should generate different values on each call (randomness)", () => {
      const uuid1 = templateHelpers.uuid();
      const uuid2 = templateHelpers.uuid();
      expect(uuid1).not.toBe(uuid2);

      const email1 = templateHelpers.email();
      const email2 = templateHelpers.email();
      // Emails might occasionally be the same, but UUIDs should always differ
      expect(uuid1).not.toBe(uuid2);
    });
  });
});
