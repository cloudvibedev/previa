# Share Access Levels Design

## Goal

Stack and pipeline sharing must support explicit permission levels instead of a single editor-only share. Owners choose whether a user can only view, execute, edit, or manage/delete/share the resource.

## Access Model

Use hierarchical access levels for both stacks and pipelines:

- `viewer`: can view metadata, definitions, specs, environment groups, and history.
- `runner`: includes viewer access and can execute tests.
- `editor`: includes runner access and can edit stack/pipeline content.
- `manager`: includes editor access and can delete the resource, share it, revoke shares, and change visibility.

Owners, root, and admin keep full access. Existing `editor` shares remain valid and keep their current behavior. Public stacks and public pipelines keep current public-write behavior for anonymous/public users; explicit shares only affect named users.

## Backend Design

`ProjectShareAccessLevel` and `PipelineShareAccessLevel` gain `viewer`, `runner`, `editor`, and `manager`. ACL services stop treating share existence as write access and instead compare the requested action to the stored access level.

Access mapping:

- Read endpoints require `viewer`.
- Execution endpoints require `runner`.
- Edit endpoints require `editor`.
- Delete/share/visibility endpoints require `manager`.

Pipeline access can inherit stack access. A stack `runner` can run pipelines in that stack. A stack `editor` can edit pipelines in that stack. A stack `manager` can share and change visibility for pipelines in that stack. Pipeline deletion still requires direct pipeline ownership, root/admin access, or a direct pipeline `manager` share so a public/nested pipeline with a different owner is not deleted accidentally through stack inheritance.

## Frontend Design

Share dialogs for stacks and pipelines expose a level selector with four labels: Ver, Executar, Editar, Gerenciar. Existing rows display the current level and allow replacing it by sharing the same user again with another level.

## Testing

Add backend regression tests for stack-level ACL and inherited pipeline ACL:

- viewer can read but cannot execute or edit.
- runner can execute but cannot edit.
- editor can edit but cannot delete/share.
- manager can delete/share/manage.

Run frontend build, Rust tests, release build, and a local smoke test against `127.0.0.1:55988`.
