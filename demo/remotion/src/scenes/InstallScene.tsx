import { useCurrentFrame, useVideoConfig, interpolate, spring } from "remotion";

export const InstallScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const headerOpacity = interpolate(frame, [0, 0.4 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const boxScale = spring({
    frame: frame - 0.4 * fps,
    fps,
    config: { damping: 15, stiffness: 100 },
  });

  const boxOpacity = interpolate(frame, [0.4 * fps, 0.8 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Typing the command
  const command = "cargo install --path .";
  const typingStart = 1.0 * fps;
  const typingEnd = 2.2 * fps;
  const charsVisible = Math.min(
    Math.floor(
      interpolate(frame, [typingStart, typingEnd], [0, command.length + 1], {
        extrapolateLeft: "clamp",
        extrapolateRight: "clamp",
      }),
    ),
    command.length,
  );
  const displayText = command.slice(0, charsVisible);

  const cursorOpacity = Math.round((frame * 2) / fps) % 2 === 0 ? 1 : 0;

  const noteOpacity = interpolate(frame, [2.5 * fps, 3 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full"
      style={{ backgroundColor: "#0d1117" }}
    >
      <p
        style={{
          fontSize: 40,
          fontWeight: 700,
          color: "#e6edf3",
          fontFamily: "system-ui, sans-serif",
          opacity: headerOpacity,
          marginBottom: 40,
        }}
      >
        One command to install
      </p>

      <div
        style={{
          transform: `scale(${Math.max(boxScale, 0)})`,
          opacity: boxOpacity,
          backgroundColor: "#161b22",
          border: "1px solid #30363d",
          borderRadius: 12,
          padding: "24px 40px",
          display: "flex",
          alignItems: "center",
          gap: 12,
        }}
      >
        <span style={{ fontSize: 28, color: "#7ee787", fontFamily: "monospace" }}>$</span>
        <span style={{ fontSize: 28, color: "#e6edf3", fontFamily: "monospace" }}>
          {displayText}
        </span>
        <span style={{ fontSize: 28, color: "#7ee787", fontFamily: "monospace", opacity: cursorOpacity }}>
          |
        </span>
      </div>

      <p
        style={{
          fontSize: 20,
          color: "#8b949e",
          fontFamily: "system-ui, sans-serif",
          marginTop: 32,
          opacity: noteOpacity,
        }}
      >
        Single binary. No runtime dependencies.
      </p>
    </div>
  );
};
