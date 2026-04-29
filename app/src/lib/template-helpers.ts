import { faker } from "@faker-js/faker/locale/pt_BR";
import { cpf, cnpj } from "cpf-cnpj-validator";

faker.seed();

export type HelperFunction = (...args: string[]) => string;

export const templateHelpers: Record<string, HelperFunction> = {
  uuid: () => faker.string.uuid(),
  email: () => faker.internet.email(),
  name: () => faker.person.fullName(),
  firstName: () => faker.person.firstName(),
  lastName: () => faker.person.lastName(),
  username: () => faker.internet.username(),
  phone: () => faker.phone.number(),
  cpf: () => cpf.generate(),
  cnpj: () => cnpj.generate(),
  company: () => faker.company.name(),
  timestamp: () => new Date().toISOString(),
  date: () => faker.date.recent().toISOString().split("T")[0],
  boolean: () => String(faker.datatype.boolean()),
  words: (count = "3") => faker.lorem.words(parseInt(count, 10) || 3),
  number: (min = "0", max = "100") => {
    const minNum = parseInt(min, 10) || 0;
    const maxNum = parseInt(max, 10) || 100;
    return String(faker.number.int({ min: minNum, max: maxNum }));
  },
  street: () => faker.location.streetAddress(),
  city: () => faker.location.city(),
  state: () => faker.location.state(),
  zipCode: () => faker.location.zipCode(),
  country: () => faker.location.country(),
  avatar: () => faker.image.avatar(),
  url: () => faker.internet.url(),
  password: () => faker.internet.password({ length: 12 }),
  text: (sentences = "3") => faker.lorem.sentences(parseInt(sentences, 10) || 3),
};

/**
 * Resolve a helper expression like "uuid" or "number 1 100"
 * Returns the resolved value or null if not a valid helper
 */
export function resolveHelper(expression: string): string | null {
  const parts = expression.trim().split(/\s+/);
  const helperName = parts[0];
  const args = parts.slice(1);

  const helper = templateHelpers[helperName];
  if (!helper) return null;

  return helper(...args);
}

/**
 * List of available helpers with descriptions for documentation
 */
export const helperDocs: Array<{ name: string; description: string; example: string }> = [
  { name: "uuid", description: "UUID v4", example: "550e8400-e29b-41d4-a716-446655440000" },
  { name: "email", description: "Email aleatório", example: "joao.silva@gmail.com" },
  { name: "name", description: "Nome completo", example: "João Silva" },
  { name: "firstName", description: "Primeiro nome", example: "João" },
  { name: "lastName", description: "Sobrenome", example: "Silva" },
  { name: "username", description: "Nome de usuário", example: "joao.silva42" },
  { name: "phone", description: "Telefone", example: "+55 11 99999-8888" },
  { name: "cpf", description: "CPF válido", example: "123.456.789-09" },
  { name: "cnpj", description: "CNPJ válido", example: "12.345.678/0001-90" },
  { name: "company", description: "Nome de empresa", example: "Tech Solutions Ltda" },
  { name: "timestamp", description: "Timestamp ISO atual", example: "2026-02-09T15:30:00.000Z" },
  { name: "date", description: "Data recente", example: "2026-02-08" },
  { name: "boolean", description: "true ou false", example: "true" },
  { name: "words N", description: "N palavras", example: "lorem ipsum dolor" },
  { name: "number MIN MAX", description: "Número no intervalo", example: "42" },
  { name: "street", description: "Endereço", example: "Rua das Flores, 123" },
  { name: "city", description: "Cidade", example: "São Paulo" },
  { name: "state", description: "Estado", example: "São Paulo" },
  { name: "zipCode", description: "CEP", example: "01234-567" },
  { name: "country", description: "País", example: "Brasil" },
  { name: "avatar", description: "URL de avatar", example: "https://..." },
  { name: "url", description: "URL aleatória", example: "https://example.com" },
  { name: "password", description: "Senha aleatória", example: "aB3$xY9#mN2!" },
  { name: "text N", description: "N frases", example: "Lorem ipsum dolor sit amet..." },
];
