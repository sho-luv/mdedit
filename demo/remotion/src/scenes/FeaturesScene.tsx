import { useCurrentFrame, useVideoConfig, interpolate, spring } from "remotion";

const features = [
  { icon: "Vi", title: "Vim Mode", desc: "Modal editing you know", isText: true },
  { icon: "||", title: "Live Preview", desc: "Side-by-side rendering", isText: true },
  { icon: "~>", title: "SSH Ready", desc: "Works everywhere", isText: true },
  { icon: "2.8", title: "Tiny Binary", desc: "MB, <50ms startup", isText: true },
];

export const FeaturesScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const headerOpacity = interpolate(frame, [0, 0.5 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full px-16"
      style={{ backgroundColor: "#0d1117" }}
    >
      <p
        style={{
          fontSize: 40,
          fontWeight: 700,
          color: "#e6edf3",
          fontFamily: "system-ui, sans-serif",
          opacity: headerOpacity,
          marginBottom: 48,
        }}
      >
        Built for the terminal
      </p>

      <div className="flex gap-8">
        {features.map((feat, i) => {
          const delay = 0.4 * fps + i * 0.5 * fps;

          const cardScale = spring({
            frame: frame - delay,
            fps,
            config: { damping: 12, stiffness: 150 },
          });

          const cardOpacity = interpolate(
            frame,
            [delay, delay + 0.3 * fps],
            [0, 1],
            { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
          );

          return (
            <div
              key={feat.title}
              style={{
                opacity: cardOpacity,
                transform: `scale(${Math.max(cardScale, 0)})`,
                backgroundColor: "#161b22",
                border: "1px solid #30363d",
                borderRadius: 16,
                padding: "32px 24px",
                width: 240,
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                gap: 12,
              }}
            >
              <span
                style={{
                  fontSize: 40,
                  fontWeight: 800,
                  color: "#7ee787",
                  fontFamily: "monospace",
                }}
              >
                {feat.icon}
              </span>
              <span
                style={{
                  fontSize: 24,
                  fontWeight: 700,
                  color: "#e6edf3",
                  fontFamily: "system-ui, sans-serif",
                }}
              >
                {feat.title}
              </span>
              <span
                style={{
                  fontSize: 16,
                  color: "#8b949e",
                  fontFamily: "system-ui, sans-serif",
                  textAlign: "center",
                }}
              >
                {feat.desc}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
};
