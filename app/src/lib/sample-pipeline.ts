import type { Pipeline } from "@/types/pipeline";

export const SAMPLE_PIPELINE: Pipeline = {
  name: "User CRUD Flow",
  description: "A complete flow to create, retrieve, update, and delete a user.",
  steps: [
    {
      id: "create_user",
      name: "Create User",
      description: "Create a new user with random data.",
      headers: { "Content-Type": "application/json" },
      method: "POST",
      url: "{{specs.users-api.url.hml}}/users",
      body: {
        name: "{{helpers.name}}",
        email: "{{helpers.email}}",
        username: "{{helpers.username}}",
      },
    },
    {
      id: "get_user",
      name: "Get User",
      description: "Retrieve the created user by ID.",
      headers: { "Content-Type": "application/json" },
      method: "GET",
      url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}",
    },
    {
      id: "update_user",
      name: "Update User",
      description: "Update the user's email address.",
      headers: { "Content-Type": "application/json" },
      method: "PUT",
      url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}",
      body: {
        name: "{{steps.get_user.name}}",
        email: "{{helpers.email}}",
      },
    },
    {
      id: "delete_user",
      name: "Delete User",
      description: "Delete the user from the system.",
      headers: { "Content-Type": "application/json" },
      method: "DELETE",
      url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}",
    },
  ],
};

export const SAMPLE_PIPELINE_JSON = JSON.stringify(SAMPLE_PIPELINE, null, 2);

export const SAMPLE_PIPELINE_YAML = `name: User CRUD Flow
description: A complete flow to create, retrieve, update, and delete a user.
steps:
  - id: create_user
    name: Create User
    description: Create a new user with random data.
    headers:
      Content-Type: application/json
    method: POST
    url: "{{specs.users-api.url.hml}}/users"
    body:
      name: "{{helpers.name}}"
      email: "{{helpers.email}}"
      username: "{{helpers.username}}"
  - id: get_user
    name: Get User
    description: Retrieve the created user by ID.
    headers:
      Content-Type: application/json
    method: GET
    url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}"
  - id: update_user
    name: Update User
    description: Update the user's email address.
    headers:
      Content-Type: application/json
    method: PUT
    url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}"
    body:
      name: "{{steps.get_user.name}}"
      email: "{{helpers.email}}"
  - id: delete_user
    name: Delete User
    description: Delete the user from the system.
    headers:
      Content-Type: application/json
    method: DELETE
    url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}"
`;
