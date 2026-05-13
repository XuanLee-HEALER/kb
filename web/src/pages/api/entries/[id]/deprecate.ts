import type { APIRoute } from "astro";
import { kb } from "../../../../lib/kb-client";

export const POST: APIRoute = async ({ params, request, redirect }) => {
  const id = params.id;
  if (!id) return new Response("missing id", { status: 400 });
  const form = await request.formData();
  const reason = (form.get("reason") as string)?.trim();
  if (!reason) return new Response("reason required", { status: 400 });
  try {
    await kb.deprecate(id, reason);
  } catch (e) {
    return new Response(`deprecate failed: ${(e as Error).message}`, { status: 500 });
  }
  return redirect(`/entry/${id}`, 303);
};
