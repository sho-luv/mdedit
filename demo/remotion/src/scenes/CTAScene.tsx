import { useCurrentFrame, useVideoConfig, interpolate, spring } from "remotion";

export const CTAScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleScale = spring({
    frame,
    fps,
    config: { damping: 12, stiffness: 100 },
  });

  const urlOpacity = interpolate(frame, [0.8 * fps, 1.3 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const urlY = interpolate(frame, [0.8 * fps, 1.3 * fps], [20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const githubOpacity = interpolate(
    frame,
    [1.5 * fps, 2 * fps],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
  );

  const pulseGlow = interpolate(
    frame % (fps * 1.5),
    [0, fps * 0.75, fps * 1.5],
    [0.3, 0.8, 0.3],
  );

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full"
      style={{ backgroundColor: "#0d1117" }}
    >
      <div style={{ transform: `scale(${titleScale})` }}>
        <span
          style={{
            fontSize: 56,
            fontWeight: 700,
            color: "#e6edf3",
            fontFamily: "system-ui, sans-serif",
          }}
        >
          Try it now
        </span>
      </div>

      <div
        style={{
          opacity: urlOpacity,
          transform: `translateY(${urlY}px)`,
          marginTop: 32,
          padding: "16px 40px",
          borderRadius: 12,
          backgroundColor: "#161b22",
          border: "1px solid #30363d",
          boxShadow: `0 0 ${40 * pulseGlow}px rgba(126,231,135,${pulseGlow})`,
        }}
      >
        <span
          style={{
            fontSize: 36,
            fontWeight: 600,
            color: "#7ee787",
            fontFamily: "monospace",
          }}
        >
          sho-luv/mdedit
        </span>
      </div>

      <div
        style={{
          opacity: githubOpacity,
          marginTop: 32,
          display: "flex",
          alignItems: "center",
          gap: 12,
        }}
      >
        <span
          style={{
            fontSize: 20,
            color: "#8b949e",
            fontFamily: "system-ui, sans-serif",
          }}
        >
          Open source on GitHub
        </span>
      </div>
    </div>
  );
};
