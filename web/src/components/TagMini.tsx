export function TagMini({ tag, href }: { tag: string; href?: string }) {
  const parts = tag.split("/");
  return (
    <a
      class="tag-mini"
      href={href ?? `/?tag=${encodeURIComponent(tag)}`}
      data-tag={tag}
      data-stop-propagation
    >
      {parts.map((p, i) => (
        <>
          {i > 0 && <span class="slash">/</span>}
          {p}
        </>
      ))}
    </a>
  );
}
