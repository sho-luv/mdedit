import { useCurrentFrame, useVideoConfig, interpolate, spring } from "remotion";

export const IntroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const logoScale = spring({
    frame,
    fps,
    config: { damping: 12, stiffness: 100 },
  });

  const taglineOpacity = interpolate(frame, [0.8 * fps, 1.5 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const taglineY = interpolate(frame, [0.8 * fps, 1.5 * fps], [20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const underlineWidth = interpolate(
    frame,
    [1.2 * fps, 2 * fps],
    [0, 100],
    {
      extrapolateLeft: "clamp",
      extrapolateRight: "clamp",
    },
  );

  // Blinking cursor effect
  const cursorOpacity = Math.round((frame * 2) / fps) % 2 === 0 ? 1 : 0;

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full"
      style={{ backgroundColor: "#0d1117" }}
    >
      <div style={{ transform: `scale(${logoScale})`, display: "flex", alignItems: "baseline" }}>
        <span
          style={{
            fontSize: 120,
            fontWeight: 300,
            color: "#8b949e",
            fontFamily: "monospace",
          }}
        >
          $
        </span>
        <span style={{ width: 20 }} />
        <span
          style={{
            fontSize: 120,
            fontWeight: 700,
            color: "#7ee787",
            fontFamily: "monospace",
          }}
        >
          mdedit
        </span>
        <span
          style={{
            fontSize: 120,
            fontWeight: 300,
            color: "#7ee787",
            fontFamily: "monospace",
            opacity: cursorOpacity,
          }}
        >
          _
        </span>
      </div>

      <div
        style={{
          opacity: taglineOpacity,
          transform: `translateY(${taglineY}px)`,
          marginTop: 16,
        }}
      >
        <p
          style={{
            fontSize: 32,
            color: "#8b949e",
            fontFamily: "system-ui, sans-serif",
            letterSpacing: 2,
          }}
        >
          Terminal Markdown Editor
        </p>
      </div>

      <div
        style={{
          width: `${underlineWidth}%`,
          maxWidth: 400,
          height: 3,
          backgroundColor: "#7ee787",
          marginTop: 24,
          borderRadius: 2,
        }}
      />
    </div>
  );
};
