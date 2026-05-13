import type { APIRoute } from "astro";
import { buildKindData } from "../../../../lib/form";
import { kb } from "../../../../lib/kb-client";

export const POST: APIRoute = async ({ params, request, redirect }) => {
  const id = params.id;
  if (!id) return new Response("missing id", { status: 400 });

  const form = await request.formData();
  const title = (form.get("title") as string)?.trim();
  const body = (form.get("body") as string) ?? "";
  const tagsRaw = (form.get("tags") as string) ?? "";
  const tags = tagsRaw
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);

  let data: ReturnType<typeof buildKindData>;
  try {
    data = buildKindData(form);
  } catch (e) {
    return new Response((e as Error).message, { status: 400 });
  }

  const partial = {
    title,
    body,
    tags,
    ...data,
  };

  try {
    await kb.update(id, partial);
  } catch (e) {
    return new Response(`update failed: ${(e as Error).message}`, { status: 500 });
  }
  return redirect(`/entry/${id}`, 303);
};
