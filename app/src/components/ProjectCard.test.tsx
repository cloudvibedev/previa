import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";

import { ProjectCard } from "@/components/ProjectCard";
import type { Project } from "@/types/project";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: { language: "en" },
    t: (key: string) => ({
      "common.open": "Open",
      "common.rename": "Rename",
      "common.duplicate": "Duplicate",
      "common.export": "Export",
      "common.delete": "Delete",
      "dashboard.title": "Dashboard",
      "projects.open": "Open Stack",
    }[key] ?? key),
  }),
}));

const project: Project = {
  id: "project-1",
  name: "Stack 1",
  createdAt: "2026-04-30T00:00:00.000Z",
  updatedAt: "2026-04-30T00:00:00.000Z",
  specs: [],
  envGroups: [],
  pipelines: [],
};

describe("ProjectCard", () => {
  function renderProjectCard(overrides: Partial<ComponentProps<typeof ProjectCard>> = {}) {
    const props = {
      project,
      onOpen: vi.fn(),
      onDashboard: vi.fn(),
      onDuplicate: vi.fn(),
      onDelete: vi.fn(),
      onExport: vi.fn(),
      ...overrides,
    };

    render(<ProjectCard {...props} />);
    return props;
  }

  async function openCardMenu() {
    const menuButton = screen.getByRole("button", { name: "Stack 1 actions" });
    fireEvent.pointerDown(menuButton);
    fireEvent.keyDown(menuButton, { key: "Enter" });
  }

  it("opens the project dashboard from the card menu", async () => {
    const onDashboard = vi.fn();

    renderProjectCard({ onDashboard });

    await openCardMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Dashboard" }));

    expect(onDashboard).toHaveBeenCalledWith("project-1");
  });

  it("duplicates the project from the card menu", async () => {
    const onDuplicate = vi.fn();

    renderProjectCard({ onDuplicate });

    await openCardMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));

    expect(onDuplicate).toHaveBeenCalledWith("project-1");
  });

  it("exports the project from the card menu", async () => {
    const onExport = vi.fn();

    renderProjectCard({ onExport });

    await openCardMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));

    expect(onExport).toHaveBeenCalledWith("project-1");
  });
});
