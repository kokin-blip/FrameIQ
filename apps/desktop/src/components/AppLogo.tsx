export function AppLogo({
  size = 32,
  className = "",
}: {
  size?: 32 | 48 | 64;
  className?: string;
}) {
  return (
    <img
      className={`app-logo ${className}`.trim()}
      src="/brand/app-icon.svg"
      alt=""
      aria-hidden="true"
      width={size}
      height={size}
      draggable={false}
    />
  );
}
