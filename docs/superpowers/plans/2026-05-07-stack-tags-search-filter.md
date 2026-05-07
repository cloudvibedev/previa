# Stack Tags Search Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persistent tags to stacks, allow tags to be created/edited per stack, and add My Stacks filtering by tags plus search by title and description.

**Architecture:** Treat stacks as existing `Project` records. Tags are project metadata (`tags: string[]`) persisted in both local IndexedDB and the backend `projects` table as JSON, while search and tag filtering stay as UI-only state inside `ProjectsPage`. Reusable tag normalization and project filtering logic live in focused frontend helper modules so tests can exercise behavior without rendering the whole page.

**Tech Stack:** Rust/Axum/SQLx migrations for backend project metadata, React/Vite/TypeScript/Vitest for the app, IndexedDB wrapper in `app/src/lib/project-db.ts`, shadcn UI primitives, lucide-react icons, i18next locale JSON files.

---

## File Structure

- Modify `app/src/types/project.ts`: add `tags?: string[]` to `Project`.
- Create `app/src/lib/project-tags.ts`: normalize tags, deduplicate tags case-insensitively, collect all project tags, and filter projects by search text plus selected tags.
- Create `app/src/lib/project-tags.test.ts`: unit tests for normalization, collection, and filtering.
- Modify `app/src/lib/project-db.ts`: persist tags in IndexedDB project rows, with old rows defaulting to `[]`.
- Modify `app/src/lib/project-db.test.ts` if present later in execution; otherwise rely on `project-tags.test.ts` plus page/store tests because current file-level IndexedDB testing is not established for project rows.
- Modify `app/src/lib/api-client.ts`: include tags in API request/record types and mapping.
- Modify `app/src/stores/useProjectStore.ts`: create, update, and duplicate projects with tags in local and remote modes.
- Modify `app/src/components/ProjectCard.tsx`: render tag badges and add an “Edit tags” menu action.
- Create `app/src/components/ProjectTagsDialog.tsx`: dialog with input, removable badges, cancel/save.
- Create `app/src/components/ProjectTagsDialog.test.tsx`: component tests for adding, deduplicating, removing, and saving tags.
- Modify `app/src/pages/ProjectsPage.tsx`: add search input, tag filter controls, filtered empty state, and tag edit dialog wiring.
- Modify `app/src/pages/ProjectsPage.test.tsx`: page tests for title/description search, tag filtering, and saving edited tags through `updateProject`.
- Modify locale files:
  - `app/src/i18n/locales/en.json`
  - `app/src/i18n/locales/pt-BR.json`
  - `app/src/i18n/locales/es.json`
  - `app/src/i18n/locales/fr.json`
  - `app/src/i18n/locales/de.json`
  - `app/src/i18n/locales/ja.json`
  - `app/src/i18n/locales/ko.json`
  - `app/src/i18n/locales/zh-CN.json`
- Modify `main/src/server/models.rs`: add `tags` to project request/response structs.
- Modify `main/src/server/db/projects.rs`: read/write `tags_json`.
- Modify project insert test helpers that create raw project rows if compiler errors identify them:
  - `main/src/server/handlers/tests_load.rs`
  - `main/src/server/handlers/tests_e2e.rs`
  - `main/src/server/handlers/tests_e2e_queue.rs`
  - `main/src/server/handlers/pipelines.rs`
  - `main/src/server/db/e2e_queues.rs`
  - `main/src/server/db/transfers.rs`
- Create migrations:
  - `main/migrations/sqlite/202605070001_add_project_tags.sql`
  - `main/migrations/postgres/202605070001_add_project_tags.sql`
  - `main/migrations/202605070001_add_project_tags.sql`

## Data Contract

Use this normalized contract everywhere:

```ts
interface Project {
  tags?: string[];
}
```

Backend JSON response:

```json
{
  "id": "project-1",
  "name": "Payments",
  "description": "Checkout and billing stack",
  "tags": ["billing", "critical"],
  "createdAt": "2026-05-07T12:00:00Z",
  "updatedAt": "2026-05-07T12:00:00Z"
}
```

Normalization rules:

- Trim each tag.
- Remove empty tags.
- Deduplicate case-insensitively.
- Preserve the first casing the user entered.
- Sort collected filter tags with `localeCompare`.
- Project filter must match all selected tags.
- Search must match `name` or `description`, case-insensitively.

---

### Task 1: Frontend Tag Helpers

**Files:**
- Create: `app/src/lib/project-tags.ts`
- Create: `app/src/lib/project-tags.test.ts`
- Modify: `app/src/types/project.ts`

- [ ] **Step 1: Write the failing helper tests**

Create `app/src/lib/project-tags.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import {
  collectProjectTags,
  filterProjectsBySearchAndTags,
  normalizeProjectTags,
} from "@/lib/project-tags";
import type { Project } from "@/types/project";

const baseProject = (project: Partial<Project>): Project => ({
  id: project.id ?? "project-1",
  name: project.name ?? "Payments",
  description: project.description,
  createdAt: "2026-05-07T00:00:00.000Z",
  updatedAt: "2026-05-07T00:00:00.000Z",
  specs: [],
  envGroups: [],
  pipelines: [],
  tags: project.tags,
});

describe("project tags", () => {
  it("normalizes tags by trimming empties and deduplicating case-insensitively", () => {
    expect(normalizeProjectTags([" billing ", "", "Billing", "Critical"])).toEqual([
      "billing",
      "Critical",
    ]);
  });

  it("collects sorted unique tags from projects", () => {
    const projects = [
      baseProject({ tags: ["critical", "billing"] }),
      baseProject({ id: "project-2", tags: ["Billing", "qa"] }),
    ];

    expect(collectProjectTags(projects)).toEqual(["billing", "critical", "qa"]);
  });

  it("searches project title and description case-insensitively", () => {
    const projects = [
      baseProject({ name: "Payments", description: "Checkout stack" }),
      baseProject({ id: "project-2", name: "Orders", description: "Fulfillment flows" }),
    ];

    expect(filterProjectsBySearchAndTags(projects, "checkout", [])).toHaveLength(1);
    expect(filterProjectsBySearchAndTags(projects, "orders", [])[0].id).toBe("project-2");
  });

  it("filters projects that contain all selected tags", () => {
    const projects = [
      baseProject({ id: "project-1", tags: ["billing", "critical"] }),
      baseProject({ id: "project-2", tags: ["billing"] }),
      baseProject({ id: "project-3", tags: ["qa", "critical"] }),
    ];

    expect(filterProjectsBySearchAndTags(projects, "", ["billing", "critical"]).map((p) => p.id)).toEqual([
      "project-1",
    ]);
  });
});
```

- [ ] **Step 2: Run the helper tests to verify they fail**

Run:

```bash
cd app && npm test -- src/lib/project-tags.test.ts
```

Expected: FAIL because `@/lib/project-tags` does not exist.

- [ ] **Step 3: Add the project tags type**

Modify `app/src/types/project.ts`:

```ts
export interface Project {
  id: string;
  name: string;
  description?: string;
  tags?: string[];
  createdAt: string;
  updatedAt: string;
  /** @deprecated Use specs[] instead. Kept for backward compatibility — returns merged routes from all specs. */
  spec?: OpenAPISpec;
  specs: ProjectSpec[];
  envGroups: ProjectEnvGroup[];
  pipelines: Pipeline[];
}
```

- [ ] **Step 4: Implement helper functions**

Create `app/src/lib/project-tags.ts`:

```ts
import type { Project } from "@/types/project";

export function normalizeProjectTags(tags: readonly string[] | undefined): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const tag of tags ?? []) {
    const trimmed = tag.trim();
    if (!trimmed) continue;

    const key = trimmed.toLocaleLowerCase();
    if (seen.has(key)) continue;

    seen.add(key);
    normalized.push(trimmed);
  }

  return normalized;
}

export function collectProjectTags(projects: readonly Project[]): string[] {
  const byKey = new Map<string, string>();

  for (const project of projects) {
    for (const tag of normalizeProjectTags(project.tags)) {
      const key = tag.toLocaleLowerCase();
      if (!byKey.has(key)) {
        byKey.set(key, tag);
      }
    }
  }

  return Array.from(byKey.values()).sort((left, right) => left.localeCompare(right));
}

export function filterProjectsBySearchAndTags(
  projects: readonly Project[],
  searchQuery: string,
  selectedTags: readonly string[],
): Project[] {
  const query = searchQuery.trim().toLocaleLowerCase();
  const selectedKeys = selectedTags.map((tag) => tag.toLocaleLowerCase());

  return projects.filter((project) => {
    const matchesSearch = !query
      || project.name.toLocaleLowerCase().includes(query)
      || (project.description ?? "").toLocaleLowerCase().includes(query);

    if (!matchesSearch) return false;

    const projectTagKeys = new Set(normalizeProjectTags(project.tags).map((tag) => tag.toLocaleLowerCase()));
    return selectedKeys.every((tag) => projectTagKeys.has(tag));
  });
}
```

- [ ] **Step 5: Run helper tests to verify they pass**

Run:

```bash
cd app && npm test -- src/lib/project-tags.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add app/src/types/project.ts app/src/lib/project-tags.ts app/src/lib/project-tags.test.ts
git commit -m "feat: add project tag helpers"
```

---

### Task 2: Backend Project Tags Persistence

**Files:**
- Create: `main/migrations/sqlite/202605070001_add_project_tags.sql`
- Create: `main/migrations/postgres/202605070001_add_project_tags.sql`
- Create: `main/migrations/202605070001_add_project_tags.sql`
- Modify: `main/src/server/models.rs`
- Modify: `main/src/server/db/projects.rs`

- [ ] **Step 1: Add migration files**

Create `main/migrations/sqlite/202605070001_add_project_tags.sql`:

```sql
ALTER TABLE projects
ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
```

Create `main/migrations/postgres/202605070001_add_project_tags.sql`:

```sql
ALTER TABLE projects
ADD COLUMN IF NOT EXISTS tags_json TEXT NOT NULL DEFAULT '[]';
```

Create `main/migrations/202605070001_add_project_tags.sql`:

```sql
ALTER TABLE projects
ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
```

- [ ] **Step 2: Write backend persistence tests**

Add tests at the bottom of `main/src/server/db/projects.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> crate::server::db::DbPool {
        let db = crate::server::db::DbPool::connect("sqlite::memory:", 1)
            .await
            .expect("sqlite memory db");
        sqlx::migrate!("./migrations/sqlite")
            .run(db.pool())
            .await
            .expect("migrations");
        db
    }

    #[tokio::test]
    async fn project_records_round_trip_tags() {
        let db = db().await;

        let project = upsert_project_metadata(
            &db,
            "project-1".to_owned(),
            ProjectMetadataUpsertRequest {
                name: "Payments".to_owned(),
                description: Some("Checkout".to_owned()),
                tags: vec!["billing".to_owned(), "critical".to_owned()],
            },
        )
        .await
        .expect("upsert project");

        assert_eq!(project.tags, vec!["billing", "critical"]);

        let loaded = load_project_record(&db, "project-1")
            .await
            .expect("load project")
            .expect("project exists");
        assert_eq!(loaded.tags, vec!["billing", "critical"]);
    }
}
```

- [ ] **Step 3: Run backend test to verify it fails**

Run:

```bash
cd main && cargo test server::db::projects::tests::project_records_round_trip_tags
```

Expected: FAIL because `ProjectMetadataUpsertRequest` and `ProjectRecord` do not have `tags`.

- [ ] **Step 4: Update backend models**

Modify `main/src/server/models.rs` project structs:

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUpsertRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[schema(value_type = Object, nullable = true)]
    pub spec: Option<Value>,
    #[serde(default)]
    pub pipelines: Vec<PipelineInput>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMetadataUpsertRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 5: Update backend DB mapping**

In `main/src/server/db/projects.rs`, add helpers near the imports:

```rust
fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_owned())
}

fn tags_from_row(row: &sqlx::any::AnyRow) -> Vec<String> {
    row.try_get::<String, _>("tags_json")
        .ok()
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default()
}
```

Update `SELECT` clauses from:

```sql
SELECT id, name, description, created_at, updated_at FROM projects
```

to:

```sql
SELECT id, name, description, tags_json, created_at, updated_at FROM projects
```

Set `tags: tags_from_row(&row)` when building every `ProjectRecord`.

Update `upsert_project_metadata` insert:

```sql
INSERT INTO projects (
    id, name, description, tags_json, created_at, updated_at, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    name = excluded.name,
    description = excluded.description,
    tags_json = excluded.tags_json,
    updated_at = excluded.updated_at,
    updated_at_ms = excluded.updated_at_ms
```

Bind `tags_to_json(&payload.tags)` after `payload.description`.

Update `upsert_project_with_pipelines` insert:

```sql
INSERT INTO projects (
    id, name, description, tags_json, created_at, updated_at, created_at_ms, updated_at_ms, spec_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    name = excluded.name,
    description = excluded.description,
    tags_json = excluded.tags_json,
    updated_at = excluded.updated_at,
    updated_at_ms = excluded.updated_at_ms,
    spec_json = excluded.spec_json
```

Bind `tags_to_json(&payload.tags)` after `payload.description`.

Update `create_project_with_pipelines` insert to include `tags_json` with `"[]"`.

- [ ] **Step 6: Fix raw project inserts found by compiler**

For each compiler error in raw SQL project inserts, add `tags_json` to the column list and bind `"[]"`. The expected pattern is:

```rust
.bind("[]")
```

after the `description` bind when the insert uses explicit `projects` columns.

- [ ] **Step 7: Run backend test to verify it passes**

Run:

```bash
cd main && cargo test server::db::projects::tests::project_records_round_trip_tags
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add main/migrations/sqlite/202605070001_add_project_tags.sql main/migrations/postgres/202605070001_add_project_tags.sql main/migrations/202605070001_add_project_tags.sql main/src/server/models.rs main/src/server/db/projects.rs
git add main/src/server/handlers/tests_load.rs main/src/server/handlers/tests_e2e.rs main/src/server/handlers/tests_e2e_queue.rs main/src/server/handlers/pipelines.rs main/src/server/db/e2e_queues.rs main/src/server/db/transfers.rs
git commit -m "feat: persist project tags in backend"
```

---

### Task 3: Frontend API and Local Persistence

**Files:**
- Modify: `app/src/lib/api-client.ts`
- Modify: `app/src/lib/project-db.ts`
- Modify: `app/src/stores/useProjectStore.ts`

- [ ] **Step 1: Write store-facing page test that will fail until tags save**

Add to `app/src/pages/ProjectsPage.test.tsx`:

```ts
it("saves edited stack tags through the project store", async () => {
  projectStoreMock.projects = [{ ...project, tags: ["billing"] }];
  renderPage();

  await openProjectMenu();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Edit tags" }));

  fireEvent.change(await screen.findByLabelText("Tag name"), { target: { value: "critical" } });
  fireEvent.click(screen.getByRole("button", { name: "Add tag" }));
  fireEvent.click(screen.getByRole("button", { name: "Save tags" }));

  await waitFor(() => {
    expect(projectStoreMock.updateProject).toHaveBeenCalledWith("project-1", {
      tags: ["billing", "critical"],
    });
  });
});
```

Add these translation keys to the test mock:

```ts
"projects.tags.add": "Add tag",
"projects.tags.edit": "Edit tags",
"projects.tags.inputLabel": "Tag name",
"projects.tags.save": "Save tags",
"projects.tags.title": "Edit stack tags",
```

- [ ] **Step 2: Run page test to verify it fails**

Run:

```bash
cd app && npm test -- src/pages/ProjectsPage.test.tsx
```

Expected: FAIL because the UI for editing tags does not exist.

- [ ] **Step 3: Update API types and mapping**

In `app/src/lib/api-client.ts`, update project types:

```ts
export interface ProjectRecord {
  id: string;
  name: string;
  description?: string | null;
  tags?: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ProjectUpsertRequest {
  name: string;
  description?: string | null;
  tags?: string[];
  spec?: Record<string, unknown> | null;
  executionBackendUrl?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface ProjectUpdateRequest {
  name: string;
  description?: string | null;
  tags?: string[];
  executionBackendUrl?: string | null;
}
```

Update `projectRecordToLocal`:

```ts
tags: r.tags ?? [],
```

- [ ] **Step 4: Update local IndexedDB project rows**

In `app/src/lib/project-db.ts`, update `ProjectRow`:

```ts
tagsJson?: string | null;
```

Update `toProjectRow`:

```ts
tagsJson: JSON.stringify(p.tags ?? []),
```

Update `fromProjectRow` before creating the project:

```ts
let tags: string[] = [];
if (row.tagsJson) {
  try {
    const parsed = JSON.parse(row.tagsJson);
    tags = Array.isArray(parsed) ? parsed.filter((tag): tag is string => typeof tag === "string") : [];
  } catch { /* ignore parse errors */ }
}
```

Set:

```ts
tags,
```

inside the returned `Project`.

Update `duplicateProject` so copied projects preserve tags:

```ts
tags: [...(project.tags ?? [])],
```

- [ ] **Step 5: Update store create/update/duplicate payloads**

In `app/src/stores/useProjectStore.ts`, include tags in remote create:

```ts
tags: data.tags ?? [],
```

when calling `api.createProject`.

Include tags in local `newProject`:

```ts
tags: data.tags ?? [],
```

When updating remote, send:

```ts
tags: updated.tags ?? [],
```

When duplicating remote, pass:

```ts
tags: source.tags ?? [],
```

and set the local duplicated `project`:

```ts
tags: source.tags ?? [],
```

- [ ] **Step 6: Run typecheck/build for app**

Run:

```bash
cd app && npm run build
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add app/src/lib/api-client.ts app/src/lib/project-db.ts app/src/stores/useProjectStore.ts app/src/pages/ProjectsPage.test.tsx
git commit -m "feat: wire project tags through persistence"
```

---

### Task 4: Tag Editing UI

**Files:**
- Create: `app/src/components/ProjectTagsDialog.tsx`
- Create: `app/src/components/ProjectTagsDialog.test.tsx`
- Modify: `app/src/components/ProjectCard.tsx`
- Modify: `app/src/pages/ProjectsPage.tsx`
- Modify locale files listed in File Structure.

- [ ] **Step 1: Write dialog tests**

Create `app/src/components/ProjectTagsDialog.test.tsx`:

```ts
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ProjectTagsDialog } from "@/components/ProjectTagsDialog";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => ({
      "common.cancel": "Cancel",
      "projects.tags.add": "Add tag",
      "projects.tags.inputLabel": "Tag name",
      "projects.tags.save": "Save tags",
      "projects.tags.title": "Edit stack tags",
    }[key] ?? key),
  }),
}));

describe("ProjectTagsDialog", () => {
  it("adds removes deduplicates and saves tags", () => {
    const onSave = vi.fn();

    render(
      <ProjectTagsDialog
        open
        projectName="Payments"
        tags={["billing"]}
        onOpenChange={() => undefined}
        onSave={onSave}
      />,
    );

    fireEvent.change(screen.getByLabelText("Tag name"), { target: { value: "Critical" } });
    fireEvent.click(screen.getByRole("button", { name: "Add tag" }));
    fireEvent.change(screen.getByLabelText("Tag name"), { target: { value: "critical" } });
    fireEvent.click(screen.getByRole("button", { name: "Add tag" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove billing" }));
    fireEvent.click(screen.getByRole("button", { name: "Save tags" }));

    expect(onSave).toHaveBeenCalledWith(["Critical"]);
  });
});
```

- [ ] **Step 2: Run dialog test to verify it fails**

Run:

```bash
cd app && npm test -- src/components/ProjectTagsDialog.test.tsx
```

Expected: FAIL because `ProjectTagsDialog` does not exist.

- [ ] **Step 3: Implement the tag dialog**

Create `app/src/components/ProjectTagsDialog.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Tag, X } from "lucide-react";

import { normalizeProjectTags } from "@/lib/project-tags";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface ProjectTagsDialogProps {
  open: boolean;
  projectName?: string;
  tags: string[];
  onOpenChange: (open: boolean) => void;
  onSave: (tags: string[]) => void;
}

export function ProjectTagsDialog({ open, projectName, tags, onOpenChange, onSave }: ProjectTagsDialogProps) {
  const { t } = useTranslation();
  const [draftTags, setDraftTags] = useState<string[]>(tags);
  const [tagName, setTagName] = useState("");

  useEffect(() => {
    if (open) {
      setDraftTags(normalizeProjectTags(tags));
      setTagName("");
    }
  }, [open, tags]);

  const normalizedDraftTags = useMemo(() => normalizeProjectTags(draftTags), [draftTags]);

  const addTag = () => {
    const next = normalizeProjectTags([...normalizedDraftTags, tagName]);
    setDraftTags(next);
    setTagName("");
  };

  const removeTag = (tagToRemove: string) => {
    const key = tagToRemove.toLocaleLowerCase();
    setDraftTags(normalizedDraftTags.filter((tag) => tag.toLocaleLowerCase() !== key));
  };

  const saveTags = () => {
    onSave(normalizedDraftTags);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("projects.tags.title")}</DialogTitle>
          {projectName && <p className="text-sm text-muted-foreground">{projectName}</p>}
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="project-tag-name">{t("projects.tags.inputLabel")}</Label>
            <div className="flex gap-2">
              <Input
                id="project-tag-name"
                value={tagName}
                onChange={(event) => setTagName(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addTag();
                  }
                }}
              />
              <Button type="button" variant="outline" onClick={addTag}>
                <Tag className="h-4 w-4" />
                {t("projects.tags.add")}
              </Button>
            </div>
          </div>

          <div className="flex min-h-9 flex-wrap gap-2">
            {normalizedDraftTags.map((tag) => (
              <Badge key={tag} variant="secondary" className="gap-1.5">
                {tag}
                <button
                  type="button"
                  className="rounded-sm text-muted-foreground hover:text-foreground"
                  aria-label={`Remove ${tag}`}
                  onClick={() => removeTag(tag)}
                >
                  <X className="h-3 w-3" />
                </button>
              </Badge>
            ))}
          </div>
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button type="button" onClick={saveTags}>
            {t("projects.tags.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 4: Update ProjectCard**

Modify `app/src/components/ProjectCard.tsx` imports:

```tsx
import { BarChart3, FolderOpen, MoreVertical, Copy, Trash2, Calendar, Download, Pencil, Tag } from "lucide-react";
```

Add prop:

```ts
onEditTags?: (id: string) => void;
```

Render tags under description/title:

```tsx
{project.tags && project.tags.length > 0 && (
  <div className="mt-2 flex flex-wrap gap-1.5">
    {project.tags.slice(0, 3).map((tag) => (
      <Badge key={tag} variant="outline" className="max-w-[120px] truncate text-xs">
        {tag}
      </Badge>
    ))}
    {project.tags.length > 3 && (
      <Badge variant="secondary" className="text-xs">
        +{project.tags.length - 3}
      </Badge>
    )}
  </div>
)}
```

Add menu item after rename:

```tsx
<DropdownMenuItem className="gap-2.5" onClick={() => onEditTags?.(project.id)}>
  <Tag className="h-4 w-4" />
  {t("projects.tags.edit")}
</DropdownMenuItem>
```

- [ ] **Step 5: Wire dialog in ProjectsPage**

In `app/src/pages/ProjectsPage.tsx`, import:

```tsx
import { ProjectTagsDialog } from "@/components/ProjectTagsDialog";
```

Add state:

```tsx
const [projectToEditTags, setProjectToEditTags] = useState<string | null>(null);
```

Add derived project:

```tsx
const tagProject = projects.find((project) => project.id === projectToEditTags);
```

Pass to `ProjectCard`:

```tsx
onEditTags={setProjectToEditTags}
```

Render dialog near other dialogs:

```tsx
<ProjectTagsDialog
  open={Boolean(tagProject)}
  projectName={tagProject?.name}
  tags={tagProject?.tags ?? []}
  onOpenChange={(open) => {
    if (!open) setProjectToEditTags(null);
  }}
  onSave={async (tags) => {
    if (!tagProject) return;
    await updateProject(tagProject.id, { tags });
    toast.success(t("projects.tags.updated"));
  }}
/>
```

- [ ] **Step 6: Add locale keys**

Add these keys to every listed locale file. English values:

```json
"projects.tags.add": "Add tag",
"projects.tags.edit": "Edit tags",
"projects.tags.inputLabel": "Tag name",
"projects.tags.save": "Save tags",
"projects.tags.title": "Edit stack tags",
"projects.tags.updated": "Stack tags updated.",
"projects.filters.searchPlaceholder": "Search title or description",
"projects.filters.clear": "Clear filters",
"projects.filters.noResults.title": "No stacks match",
"projects.filters.noResults.description": "Try another search or remove a tag filter."
```

Portuguese values:

```json
"projects.tags.add": "Adicionar tag",
"projects.tags.edit": "Editar tags",
"projects.tags.inputLabel": "Nome da tag",
"projects.tags.save": "Salvar tags",
"projects.tags.title": "Editar tags da stack",
"projects.tags.updated": "Tags da stack atualizadas.",
"projects.filters.searchPlaceholder": "Buscar título ou descrição",
"projects.filters.clear": "Limpar filtros",
"projects.filters.noResults.title": "Nenhuma stack encontrada",
"projects.filters.noResults.description": "Tente outra busca ou remova um filtro de tag."
```

For other locales, use clear English fallback values if translation quality is uncertain. The UI already tolerates mixed-language keys better than missing keys.

- [ ] **Step 7: Run dialog and page tests**

Run:

```bash
cd app && npm test -- src/components/ProjectTagsDialog.test.tsx src/pages/ProjectsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add app/src/components/ProjectTagsDialog.tsx app/src/components/ProjectTagsDialog.test.tsx app/src/components/ProjectCard.tsx app/src/pages/ProjectsPage.tsx app/src/pages/ProjectsPage.test.tsx app/src/i18n/locales/*.json
git commit -m "feat: add stack tag editing UI"
```

---

### Task 5: My Stacks Search and Tag Filters

**Files:**
- Modify: `app/src/pages/ProjectsPage.tsx`
- Modify: `app/src/pages/ProjectsPage.test.tsx`

- [ ] **Step 1: Add page tests for search and tag filters**

Add tests to `app/src/pages/ProjectsPage.test.tsx`:

```ts
it("filters stacks by title and description search", async () => {
  projectStoreMock.projects = [
    { ...project, id: "project-1", name: "Payments", description: "Checkout flows" },
    { ...project, id: "project-2", name: "Orders", description: "Fulfillment APIs" },
  ];

  renderPage();

  fireEvent.change(screen.getByPlaceholderText("Search title or description"), {
    target: { value: "checkout" },
  });

  expect(screen.getByText("Payments")).toBeInTheDocument();
  expect(screen.queryByText("Orders")).not.toBeInTheDocument();
});

it("filters stacks by selected tag", async () => {
  projectStoreMock.projects = [
    { ...project, id: "project-1", name: "Payments", tags: ["billing"] },
    { ...project, id: "project-2", name: "Orders", tags: ["fulfillment"] },
  ];

  renderPage();

  fireEvent.click(screen.getByRole("button", { name: "billing" }));

  expect(screen.getByText("Payments")).toBeInTheDocument();
  expect(screen.queryByText("Orders")).not.toBeInTheDocument();
});
```

Add mock translation keys:

```ts
"projects.filters.searchPlaceholder": "Search title or description",
"projects.filters.clear": "Clear filters",
"projects.filters.noResults.title": "No stacks match",
"projects.filters.noResults.description": "Try another search or remove a tag filter.",
```

- [ ] **Step 2: Run page tests to verify they fail**

Run:

```bash
cd app && npm test -- src/pages/ProjectsPage.test.tsx
```

Expected: FAIL because the search input and tag buttons do not exist.

- [ ] **Step 3: Implement filters in ProjectsPage**

Modify imports:

```tsx
import { Search, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { collectProjectTags, filterProjectsBySearchAndTags } from "@/lib/project-tags";
```

Add state:

```tsx
const [searchQuery, setSearchQuery] = useState("");
const [selectedTags, setSelectedTags] = useState<string[]>([]);
```

Add derived values:

```tsx
const availableTags = useMemo(() => collectProjectTags(projects), [projects]);
const filteredProjects = useMemo(
  () => filterProjectsBySearchAndTags(projects, searchQuery, selectedTags),
  [projects, searchQuery, selectedTags],
);
const hasFilters = searchQuery.trim().length > 0 || selectedTags.length > 0;
```

Add tag toggle:

```tsx
const toggleTagFilter = (tag: string) => {
  setSelectedTags((current) => (
    current.includes(tag)
      ? current.filter((item) => item !== tag)
      : [...current, tag]
  ));
};
```

Add filter UI below the page header and above the loading/list block:

```tsx
{projects.length > 0 && (
  <div className="mb-5 space-y-3">
    <div className="relative">
      <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        value={searchQuery}
        onChange={(event) => setSearchQuery(event.target.value)}
        placeholder={t("projects.filters.searchPlaceholder")}
        className="pl-9"
      />
    </div>

    {availableTags.length > 0 && (
      <div className="flex flex-wrap gap-2">
        {availableTags.map((tag) => {
          const selected = selectedTags.includes(tag);
          return (
            <Button
              key={tag}
              type="button"
              variant={selected ? "default" : "outline"}
              size="sm"
              className="h-7 px-2 text-xs"
              onClick={() => toggleTagFilter(tag)}
            >
              {tag}
            </Button>
          );
        })}
        {hasFilters && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            onClick={() => {
              setSearchQuery("");
              setSelectedTags([]);
            }}
          >
            <X className="h-3.5 w-3.5" />
            {t("projects.filters.clear")}
          </Button>
        )}
      </div>
    )}
  </div>
)}
```

Change list rendering from `projects.map` to `filteredProjects.map`.

Change condition from `projects.length > 0` to `filteredProjects.length > 0` inside the non-loading list branch. Preserve the original empty state for `projects.length === 0`.

Add filtered empty state:

```tsx
{projects.length > 0 && filteredProjects.length === 0 && (
  <div className="flex flex-col items-center justify-center rounded-lg border border-dashed border-border/50 py-12 sm:py-16 px-4 animate-fade-in">
    <Badge variant="outline" className="mb-4">
      {selectedTags.join(", ") || searchQuery}
    </Badge>
    <h3 className="text-lg font-semibold mb-2">{t("projects.filters.noResults.title")}</h3>
    <p className="text-muted-foreground mb-6 text-center max-w-sm text-sm sm:text-base">
      {t("projects.filters.noResults.description")}
    </p>
    <Button
      type="button"
      variant="outline"
      onClick={() => {
        setSearchQuery("");
        setSelectedTags([]);
      }}
    >
      <X className="h-4 w-4" />
      {t("projects.filters.clear")}
    </Button>
  </div>
)}
```

- [ ] **Step 4: Run page tests**

Run:

```bash
cd app && npm test -- src/pages/ProjectsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add app/src/pages/ProjectsPage.tsx app/src/pages/ProjectsPage.test.tsx
git commit -m "feat: filter stacks by search and tags"
```

---

### Task 6: Full Verification, Release Build, Commit, Push

**Files:**
- Verify all files changed by previous tasks.

- [ ] **Step 1: Run focused frontend tests**

Run:

```bash
cd app && npm test -- src/lib/project-tags.test.ts src/components/ProjectTagsDialog.test.tsx src/pages/ProjectsPage.test.tsx
```

Expected: PASS.

- [ ] **Step 2: Run frontend build**

Run:

```bash
cd app && npm run build
```

Expected: PASS.

- [ ] **Step 3: Run focused backend tests**

Run:

```bash
cd main && cargo test server::db::projects::tests::project_records_round_trip_tags
```

Expected: PASS.

- [ ] **Step 4: Run workspace release build required by AGENTS.md**

Run from repository root:

```bash
cargo build --release
```

Expected: PASS.

- [ ] **Step 5: Review the git diff**

Run:

```bash
git diff --stat
git diff -- app/src/types/project.ts app/src/lib/project-tags.ts app/src/lib/project-tags.test.ts app/src/lib/api-client.ts app/src/lib/project-db.ts app/src/stores/useProjectStore.ts app/src/components/ProjectCard.tsx app/src/components/ProjectTagsDialog.tsx app/src/components/ProjectTagsDialog.test.tsx app/src/pages/ProjectsPage.tsx app/src/pages/ProjectsPage.test.tsx main/src/server/models.rs main/src/server/db/projects.rs main/migrations/sqlite/202605070001_add_project_tags.sql main/migrations/postgres/202605070001_add_project_tags.sql main/migrations/202605070001_add_project_tags.sql
```

Expected: Diff only contains stack tags/search/filter work.

- [ ] **Step 6: Commit remaining changes**

Run:

```bash
git status --short
git add app/src/types/project.ts app/src/lib/project-tags.ts app/src/lib/project-tags.test.ts app/src/lib/api-client.ts app/src/lib/project-db.ts app/src/stores/useProjectStore.ts app/src/components/ProjectCard.tsx app/src/components/ProjectTagsDialog.tsx app/src/components/ProjectTagsDialog.test.tsx app/src/pages/ProjectsPage.tsx app/src/pages/ProjectsPage.test.tsx app/src/i18n/locales/*.json main/src/server/models.rs main/src/server/db/projects.rs main/migrations/sqlite/202605070001_add_project_tags.sql main/migrations/postgres/202605070001_add_project_tags.sql main/migrations/202605070001_add_project_tags.sql
git commit -m "feat: add stack tags and filters"
```

Expected: Commit succeeds. Do not stage unrelated existing dirty files unless they are required by compiler fixes for raw project inserts.

- [ ] **Step 7: Push**

Run:

```bash
git push
```

Expected: Push succeeds to the current remote branch.

---

## Self-Review

- Spec coverage: persistent tags are covered in Tasks 2 and 3; tag creation/editing is covered in Task 4; search by title/description and tag filtering are covered in Tasks 1 and 5; release build, commit, and push are covered in Task 6.
- Placeholder scan: no placeholder markers or open-ended “add tests later” steps remain.
- Type consistency: all frontend uses `tags?: string[]`; backend requests and records use `tags: Vec<String>` with `#[serde(default)]` on request payloads; database stores `tags_json` as JSON text.
