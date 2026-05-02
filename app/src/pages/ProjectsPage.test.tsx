import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import ProjectsPage from "@/pages/ProjectsPage";
import type { Project } from "@/types/project";

const useAppHeaderMock = vi.hoisted(() => vi.fn());
const exportProjectsSqliteMock = vi.hoisted(() => vi.fn());
const importProjectFileMock = vi.hoisted(() => vi.fn());
const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());

const project: Project = {
  id: "project-1",
  name: "Stack 1",
  createdAt: "2026-04-30T00:00:00.000Z",
  updatedAt: "2026-04-30T00:00:00.000Z",
  specs: [],
  envGroups: [],
  pipelines: [],
};

const projectStoreMock = vi.hoisted(() => ({
  projects: [] as Project[],
  loading: false,
  loadProjects: vi.fn(),
  createProject: vi.fn(),
  updateProject: vi.fn(),
  deleteProject: vi.fn(),
  duplicateProject: vi.fn(),
}));

const useOrchestratorStoreMock = vi.hoisted(() => {
  const state = {
    url: "http://127.0.0.1:5588",
    fetchInfo: vi.fn(),
  };
  const store = vi.fn((selector: (value: typeof state) => unknown) => selector(state));
  return Object.assign(store, {
    getState: vi.fn(() => state),
  });
});

vi.mock("@/components/AppShell", () => ({
  useAppHeader: useAppHeaderMock,
}));

vi.mock("@/lib/project-io", () => ({
  exportProjectsSqlite: exportProjectsSqliteMock,
  importProjectFile: importProjectFileMock,
}));

vi.mock("@/stores/useProjectStore", () => ({
  useProjectStore: () => projectStoreMock,
}));

vi.mock("@/stores/useOrchestratorStore", () => ({
  useOrchestratorStore: useOrchestratorStoreMock,
}));

vi.mock("sonner", () => ({
  toast: {
    success: toastSuccessMock,
    error: toastErrorMock,
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: { language: "en" },
    t: (key: string, params?: Record<string, number | string>) => {
      const translations: Record<string, string> = {
        "common.delete": "Delete",
        "common.duplicate": "Duplicate",
        "common.export": "Export",
        "common.import": "Import",
        "common.open": "Open",
        "common.rename": "Rename",
        "dashboard.title": "Dashboard",
        "export.sqlite.error": "Error exporting projects.",
        "export.sqlite.success": "Projects exported successfully!",
        "projects.defaultName": `Stack ${params?.number ?? ""}`,
        "projects.duplicated": "Project duplicated!",
        "projects.empty.button": "Create First Stack",
        "projects.empty.description": "Create your first stack.",
        "projects.empty.title": "No stacks yet",
        "projects.importError": "Error importing project.",
        "projects.imported": "Project imported!",
        "projects.loading": "Loading...",
        "projects.new": "New Stack",
        "projects.open": "Open Stack",
        "projects.renamed": "Project renamed!",
        "projects.subtitle": "Manage your API stacks and pipelines",
        "projects.title": "My Stacks",
        "projects.deleteConfirm.description": `Delete ${params?.name ?? ""}?`,
        "projects.deleteConfirm.title": "Delete stack?",
      };
      return translations[key] ?? key;
    },
  }),
}));

function renderPage() {
  return render(
    <MemoryRouter>
      <ProjectsPage />
    </MemoryRouter>,
  );
}

async function openProjectMenu() {
  const menuButton = screen.getByRole("button", { name: "Stack 1 actions" });
  fireEvent.pointerDown(menuButton);
  fireEvent.keyDown(menuButton, { key: "Enter" });
}

describe("ProjectsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projectStoreMock.projects = [project];
    projectStoreMock.loading = false;
    projectStoreMock.createProject.mockResolvedValue(project);
    projectStoreMock.updateProject.mockResolvedValue(project);
    projectStoreMock.deleteProject.mockResolvedValue(undefined);
    projectStoreMock.duplicateProject.mockResolvedValue({
      ...project,
      id: "project-copy",
      name: "Stack 1 (cópia)",
    });
    exportProjectsSqliteMock.mockResolvedValue(undefined);
  });

  it("exports a stack card as a SQLite project export", async () => {
    renderPage();

    await openProjectMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));

    await waitFor(() => {
      expect(exportProjectsSqliteMock).toHaveBeenCalledWith(["project-1"], false, false);
    });
    expect(toastSuccessMock).toHaveBeenCalledWith("Projects exported successfully!");
  });

  it("refreshes projects after duplicating from the stack card", async () => {
    renderPage();

    await waitFor(() => expect(projectStoreMock.loadProjects).toHaveBeenCalled());
    projectStoreMock.loadProjects.mockClear();

    await openProjectMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));

    await waitFor(() => {
      expect(projectStoreMock.duplicateProject).toHaveBeenCalledWith("project-1");
    });
    expect(projectStoreMock.loadProjects).toHaveBeenCalledTimes(1);
    expect(toastSuccessMock).toHaveBeenCalledWith("Project duplicated!");
  });
});
