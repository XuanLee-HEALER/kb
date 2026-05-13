// The Sakya three-stripe visual armature. Vertical (3px wide) or horizontal
// (3px tall) variant.

export function Stripe3({ vertical = true }: { vertical?: boolean }) {
  const cls = vertical ? "stripe3" : "stripe3-h";
  return (
    <span class={cls} aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  );
}
